/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * This file is part of M3 (Microkernel-based SysteM for Heterogeneous Manycores).
 *
 * M3 is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License version 2 as
 * published by the Free Software Foundation.
 *
 * M3 is distributed in the hope that it will be useful, but
 * WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * General Public License version 2 for more details.
 */

#![no_std]

#[macro_use]
extern crate m3;
#[macro_use]
extern crate bitflags;

mod addrspace;
mod dataspace;
mod mapper;
mod physmem;
mod regions;

use m3::cap::Selector;
use m3::cell::LazyStaticCell;
use m3::col::{String, ToString, Vec};
use m3::com::{RecvGate, SGateArgs, SendGate};
use m3::env;
use m3::errors::{Code, Error};
use m3::kif;
use m3::pes::{Activity, ExecActivity, VPEArgs, PE, VPE};
use m3::serialize::{Sink, Source};
use m3::server::{
    server_loop, CapExchange, Handler, RequestHandler, Server, SessId, SessionContainer,
    DEF_MAX_CLIENTS,
};
use m3::session::{ClientSession, Pager, PagerDelOp, PagerOp};
use m3::tcu::{Label, TCUIf};
use m3::vfs;

use addrspace::AddrSpace;

pub const LOG_DEF: bool = false;

static REQHDL: LazyStaticCell<RequestHandler> = LazyStaticCell::default();

struct PagerReqHandler {
    sel: Selector,
    sessions: SessionContainer<AddrSpace>,
}

impl Handler<AddrSpace> for PagerReqHandler {
    fn sessions(&mut self) -> &mut m3::server::SessionContainer<AddrSpace> {
        &mut self.sessions
    }

    fn open(
        &mut self,
        crt: usize,
        srv_sel: Selector,
        _arg: &str,
    ) -> Result<(Selector, SessId), Error> {
        self.sessions.add_next(crt, srv_sel, false, |sess| {
            log!(crate::LOG_DEF, "[{}] pager::open()", sess.ident());
            Ok(AddrSpace::new(crt, sess, None))
        })
    }

    fn obtain(&mut self, crt: usize, sid: SessId, xchg: &mut CapExchange) -> Result<(), Error> {
        if xchg.in_caps() != 1 {
            return Err(Error::new(Code::InvArgs));
        }

        let aspace = self.sessions.get_mut(sid).unwrap();
        let sel = if xchg.in_args().size() == 0 {
            aspace.add_sgate(REQHDL.recv_gate())
        }
        else {
            let sid = aspace.id();
            self.sessions
                .add_next(crt, self.sel, false, |sess| {
                    let nsid = sess.ident();
                    log!(crate::LOG_DEF, "[{}] pager::new_sess(nsid={})", sid, nsid);
                    Ok(AddrSpace::new(crt, sess, Some(sid)))
                })
                .and_then(|(sel, _)| Ok(sel))
        }?;

        xchg.out_caps(kif::CapRngDesc::new(kif::CapType::OBJECT, sel, 1));
        Ok(())
    }

    fn delegate(&mut self, _crt: usize, sid: SessId, xchg: &mut CapExchange) -> Result<(), Error> {
        if xchg.in_caps() != 1 {
            return Err(Error::new(Code::InvArgs));
        }

        let aspace = self.sessions.get_mut(sid).unwrap();
        let sel = if !aspace.has_owner() {
            let sels = VPE::cur().alloc_sel();
            aspace.init(sels);
            sels
        }
        else {
            let mut args = xchg.in_args();
            let op = args.pop_word()? as u32;

            let (sel, virt) = if op == PagerDelOp::DATASPACE.val {
                aspace.map_ds(&mut args)
            }
            else {
                aspace.map_mem(&mut args)
            }?;

            xchg.out_args().push_word(virt);
            sel
        };

        xchg.out_caps(kif::CapRngDesc::new(
            kif::CapType::OBJECT,
            sel,
            xchg.in_caps() as u32,
        ));
        Ok(())
    }

    fn close(&mut self, _crt: usize, sid: SessId) {
        log!(crate::LOG_DEF, "[{}] pager::close()", sid);
        let crt = self.sessions.get(sid).unwrap().creator();
        self.sessions.remove(crt, sid);
        // ignore all potentially outstanding messages of this session
        REQHDL.recv_gate().drop_msgs_with(sid as Label);
    }
}

fn start_child(hdl: &mut PagerReqHandler, args: &[String], share_pe: bool) -> ExecActivity {
    // create session for child
    let (sel, sid) = hdl.open(0, hdl.sel, "").expect("Session creation failed");
    let sess = ClientSession::new_bind(sel);
    #[allow(clippy::identity_conversion)]
    let sgate = SendGate::new_with(
        SGateArgs::new(REQHDL.recv_gate())
            .credits(1)
            .label(Label::from(sid as u32)),
    )
    .expect("Unable to create SendGate");

    // determine PE for child
    let pe = if !share_pe || !VPE::cur().pe_desc().has_virtmem() {
        PE::new(VPE::cur().pe_desc()).expect("Unable to allocate PE")
    }
    else {
        VPE::cur().pe().clone()
    };

    // create child VPE
    let pager = Pager::new(sess, sgate).expect("Unable to create pager");
    let mut vpe =
        VPE::new_with(pe, VPEArgs::new(&args[0]).pager(pager)).expect("Unable to create VPE");

    // pass root FS to child
    vpe.mounts()
        .add("/", VPE::cur().mounts().get_by_path("/").unwrap())
        .unwrap();
    vpe.obtain_mounts().unwrap();

    // init address space (give it VPE and mgate selector)
    let mut aspace = hdl.sessions.get_mut(sid).unwrap();
    aspace.init(vpe.sel());

    // start VPE
    let file = vfs::VFS::open(&args[0], vfs::OpenFlags::RX).expect("Unable to open binary");
    let mut mapper = mapper::ChildMapper::new(&mut aspace, vpe.pe_desc().has_virtmem());
    vpe.exec_file(&mut mapper, file, &args)
        .expect("Unable to execute child VPE")
}

#[no_mangle]
pub fn main() -> i32 {
    vfs::VFS::mount("/", "m3fs", "m3fs").expect("Unable to mount root filesystem");

    let mut skip = 0;
    let mut share_pe = false;
    let args = env::args()
        .skip(1)
        .map(|s| s.to_string())
        .collect::<Vec<String>>();
    for a in &args {
        if a == "-s" {
            share_pe = true;
            skip += 1;
            break;
        }
    }

    let args = &args[skip..];

    // create server
    let mut hdl = PagerReqHandler {
        sel: 0,
        sessions: SessionContainer::new(DEF_MAX_CLIENTS),
    };
    let s = Server::new_private("pager", &mut hdl).expect("Unable to create service");
    hdl.sel = s.sel();
    REQHDL.set(RequestHandler::default().expect("Unable to create request handler"));

    // start child
    let vpe_act = start_child(&mut hdl, args, share_pe);

    // start waiting for the child
    vpe_act
        .wait_async(1)
        .expect("Unable to start waiting for child VPE");

    let upcall_rg = RecvGate::upcall();
    server_loop(|| {
        // fetch upcalls to see whether our child died
        let msg = TCUIf::fetch_msg(upcall_rg);
        if let Some(msg) = msg {
            let upcall = msg.get_data::<kif::upcalls::VPEWait>();
            if upcall.exitcode != 0 {
                println!("Child '{}' exited with exitcode {}", &args[0], {
                    upcall.exitcode
                });
            }
            assert!(upcall.vpe_sel as Selector == vpe_act.vpe().sel());
            return Err(Error::new(Code::VPEGone));
        }

        s.handle_ctrl_chan(&mut hdl)?;

        REQHDL.get_mut().handle(|op, mut is| {
            let sid = is.label() as SessId;

            // clone is special, because we need two sessions
            if op == PagerOp::CLONE {
                let pid = hdl.sessions.get(sid).unwrap().parent();
                if let Some(pid) = pid {
                    let (sess, psess) = hdl.sessions.get_two_mut(sid, pid);
                    let sess = sess.unwrap();
                    sess.clone(is, psess.unwrap())
                }
                else {
                    Err(Error::new(Code::InvArgs))
                }
            }
            else {
                let aspace = hdl.sessions.get_mut(sid).unwrap();

                match op {
                    PagerOp::PAGEFAULT => aspace.pagefault(&mut is),
                    PagerOp::MAP_ANON => aspace.map_anon(&mut is),
                    PagerOp::UNMAP => aspace.unmap(&mut is),
                    PagerOp::CLOSE => aspace.close(&mut is).and_then(|_| {
                        hdl.close(0, is.label() as SessId);
                        Ok(())
                    }),
                    _ => Err(Error::new(Code::InvArgs)),
                }
            }
        })
    })
    .ok();

    0
}
