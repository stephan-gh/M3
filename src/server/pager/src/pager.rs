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
use m3::cell::StaticCell;
use m3::col::{String, ToString, Vec};
use m3::com::{GateIStream, RecvGate, SGateArgs, SendGate};
use m3::env;
use m3::errors::{Code, Error};
use m3::kif;
use m3::math;
use m3::pes::{Activity, VPEArgs, PE, VPE};
use m3::serialize::{Sink, Source};
use m3::server::{server_loop, CapExchange, Handler, Server, SessId, SessionContainer};
use m3::session::{ClientSession, Pager, PagerDelOp, PagerOp};
use m3::tcu::{Label, TCUIf};
use m3::vfs;

use addrspace::AddrSpace;

pub const LOG_DEF: bool = false;

const MSG_SIZE: usize = 64;
const MAX_CLIENTS: usize = 32;

static RGATE: StaticCell<Option<RecvGate>> = StaticCell::new(None);

fn rgate() -> &'static RecvGate {
    RGATE.get().as_ref().unwrap()
}

struct PagerReqHandler {
    sel: Selector,
    sessions: SessionContainer<AddrSpace>,
}

impl PagerReqHandler {
    pub fn new(sel: Selector) -> Result<Self, Error> {
        Ok(PagerReqHandler {
            sel,
            sessions: SessionContainer::new(MAX_CLIENTS),
        })
    }

    pub fn handle(&mut self, mut is: &mut GateIStream) -> Result<(), Error> {
        let op: PagerOp = is.pop()?;
        let sid = is.label() as SessId;

        let res = if op == PagerOp::CLONE {
            let pid = self.sessions.get(sid).unwrap().parent();
            if let Some(pid) = pid {
                let (sess, psess) = self.sessions.get_two_mut(sid, pid);
                let sess = sess.unwrap();
                sess.clone(is, psess.unwrap())
            }
            else {
                Err(Error::new(Code::InvArgs))
            }
        }
        else {
            let aspace = self.sessions.get_mut(sid).unwrap();

            match op {
                PagerOp::PAGEFAULT => aspace.pagefault(&mut is),
                PagerOp::MAP_ANON => aspace.map_anon(&mut is),
                PagerOp::UNMAP => aspace.unmap(&mut is),
                PagerOp::CLOSE => aspace
                    .close(&mut is)
                    .and_then(|_| Ok(self.close(is.label() as SessId))),
                _ => Err(Error::new(Code::InvArgs)),
            }
        };

        if let Err(e) = res {
            is.reply_error(e.code()).ok();
        }

        Ok(())
    }
}

impl Handler for PagerReqHandler {
    fn open(&mut self, srv_sel: Selector, _arg: &str) -> Result<(Selector, SessId), Error> {
        let sid = self.sessions.next_id()?;
        let sel = VPE::cur().alloc_sel();
        let aspace = AddrSpace::new(sid, None, srv_sel, sel)?;
        self.sessions.add(sid, aspace);
        log!(crate::LOG_DEF, "[{}] pager::open()", sid);
        Ok((sel, sid))
    }

    fn obtain(&mut self, sid: SessId, xchg: &mut CapExchange) -> Result<(), Error> {
        if xchg.in_caps() != 1 {
            return Err(Error::new(Code::InvArgs));
        }

        let aspace = self.sessions.get_mut(sid).unwrap();
        let sel = if xchg.in_args().size() == 0 {
            aspace.add_sgate()
        }
        else {
            let nsid = self.sessions.next_id()?;
            let sel = VPE::cur().alloc_sel();
            log!(crate::LOG_DEF, "[{}] pager::new_sess(nsid={})", sid, nsid);
            let aspace = AddrSpace::new(nsid, Some(sid), self.sel, sel)?;
            self.sessions.add(nsid, aspace);
            Ok(sel)
        }?;

        xchg.out_caps(kif::CapRngDesc::new(kif::CapType::OBJECT, sel, 1));
        Ok(())
    }

    fn delegate(&mut self, sid: SessId, xchg: &mut CapExchange) -> Result<(), Error> {
        if xchg.in_caps() != 1 && xchg.in_caps() != 2 {
            return Err(Error::new(Code::InvArgs));
        }

        let aspace = self.sessions.get_mut(sid).unwrap();
        let sel = if !aspace.has_as_mem() {
            let sels = VPE::cur().alloc_sels(2);
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

    fn close(&mut self, sid: SessId) {
        log!(crate::LOG_DEF, "[{}] pager::close()", sid);
        self.sessions.remove(sid);
        // ignore all potentially outstanding messages of this session
        rgate().drop_msgs_with(sid as Label);
    }
}

#[no_mangle]
pub fn main() -> i32 {
    vfs::VFS::mount("/", "m3fs", "m3fs").expect("Unable to mount root filesystem");

    let args = env::args()
        .skip(1)
        .map(|s| s.to_string())
        .collect::<Vec<String>>();
    let name = args[0].clone();

    let s = Server::new_private("pager").expect("Unable to create service");

    let mut rg = RecvGate::new(
        math::next_log2(MAX_CLIENTS * MSG_SIZE),
        math::next_log2(MSG_SIZE),
    )
    .expect("Unable to create rgate");
    rg.activate().expect("Unable to activate rgate");

    let mut hdl = PagerReqHandler::new(s.sel()).expect("Unable to create handler");

    // create session for child
    let (sel, sid) = hdl.open(s.sel(), "").expect("Session creation failed");
    let sess = ClientSession::new_bind(sel);
    let sgate = SendGate::new_with(
        SGateArgs::new(&rg)
            .credits(1)
            .label(Label::from(sid as u32)),
    )
    .expect("Unable to create SendGate");

    // create child VPE
    let pe = PE::new(VPE::cur().pe_desc()).expect("Unable to allocate PE");
    let pager = Pager::new(sess, sgate).expect("Unable to create pager");
    let mut vpe =
        VPE::new_with(pe, VPEArgs::new(&name).pager(pager)).expect("Unable to create VPE");

    // pass root FS to child
    vpe.mounts()
        .add("/", VPE::cur().mounts().get_by_path("/").unwrap())
        .unwrap();
    vpe.obtain_mounts().unwrap();

    let vpe_act = {
        // init address space (give it VPE and mgate selector)
        let mut aspace = hdl.sessions.get_mut(sid).unwrap();
        aspace.init(vpe.sel());

        // start VPE
        let file = vfs::VFS::open(&name, vfs::OpenFlags::RX).expect("Unable to open binary");
        let mut mapper = mapper::ChildMapper::new(&mut aspace, vpe.pe_desc().has_virtmem());
        vpe.exec_file(&mut mapper, file, &args)
            .expect("Unable to execute child VPE")
    };

    RGATE.set(Some(rg));

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
                println!("Child '{}' exited with exitcode {}", name, {
                    upcall.exitcode
                });
            }
            assert!(upcall.vpe_sel as Selector == vpe_act.vpe().sel());
            return Err(Error::new(Code::VPEGone));
        }

        s.handle_ctrl_chan(&mut hdl)?;

        if let Some(mut is) = rgate().fetch() {
            hdl.handle(&mut is)
        }
        else {
            Ok(())
        }
    })
    .ok();

    0
}
