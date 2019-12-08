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
mod physmem;
mod regions;

use m3::cap::Selector;
use m3::cell::StaticCell;
use m3::col::Vec;
use m3::com::{GateIStream, RecvGate};
use m3::dtu::EpId;
use m3::dtu::Label;
use m3::env;
use m3::errors::{Code, Error};
use m3::kif;
use m3::math;
use m3::pes::VPE;
use m3::server::{server_loop, Handler, Server, SessId, SessionContainer};
use m3::session::{PagerDelOp, PagerOp};

use addrspace::AddrSpace;

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
        let op: PagerOp = is.pop();
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
                PagerOp::CLOSE => {
                    aspace.close(&mut is).and_then(|_| Ok(self.close(is.label() as SessId)))
                },
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
        log!(PAGER, "[{}] pager::open()", sid);
        Ok((sel, sid))
    }

    fn obtain(&mut self, sid: SessId, data: &mut kif::service::ExchangeData) -> Result<(), Error> {
        if data.caps != 1 {
            return Err(Error::new(Code::InvArgs));
        }

        let aspace = self.sessions.get_mut(sid).unwrap();
        let sel = if data.args.count == 0 {
            aspace.add_sgate()
        }
        else {
            let nsid = self.sessions.next_id()?;
            let sel = VPE::cur().alloc_sel();
            log!(PAGER, "[{}] pager::new_sess(nsid={})", sid, nsid);
            let aspace = AddrSpace::new(nsid, Some(sid), self.sel, sel)?;
            self.sessions.add(nsid, aspace);
            Ok(sel)
        }?;

        data.caps = kif::CapRngDesc::new(kif::CapType::OBJECT, sel, 1).value();
        Ok(())
    }

    fn delegate(
        &mut self,
        sid: SessId,
        data: &mut kif::service::ExchangeData,
    ) -> Result<(), Error> {
        if data.caps != 1 && data.caps != 2 {
            return Err(Error::new(Code::InvArgs));
        }

        let aspace = self.sessions.get_mut(sid).unwrap();
        let sel = if !aspace.has_as_mem() {
            aspace.init()
        }
        else {
            let (sel, virt) = if data.args.ival(0) as u32 == PagerDelOp::DATASPACE.val {
                aspace.map_ds(&data.args)
            }
            else {
                aspace.map_mem(&data.args)
            }?;

            data.args.count = 1;
            data.args.set_ival(0, virt);
            sel
        };

        data.caps = kif::CapRngDesc::new(kif::CapType::OBJECT, sel, data.caps as u32).value();
        Ok(())
    }

    fn close(&mut self, sid: SessId) {
        log!(PAGER, "[{}] pager::close()", sid);
        self.sessions.remove(sid);
        // ignore all potentially outstanding messages of this session
        rgate().drop_msgs_with(sid as Label);
    }
}

#[no_mangle]
pub fn main() -> i32 {
    let mut sel_ep = None;

    let args: Vec<&str> = env::args().collect();
    for i in 1..args.len() {
        if args[i] == "-s" {
            let mut parts = args[i + 1].split_whitespace();
            let sel = parts.next().unwrap().parse::<Selector>().unwrap();
            let ep = parts.next().unwrap().parse::<EpId>().unwrap();
            sel_ep = Some((sel, ep));
        }
    }

    let s = if let Some(sel_ep) = sel_ep {
        Server::new_bind(sel_ep.0, sel_ep.1)
    }
    else {
        Server::new("pager").expect("Unable to create service 'pager'")
    };

    let mut hdl = PagerReqHandler::new(s.sel()).expect("Unable to create handler");

    let mut rg = RecvGate::new(
        math::next_log2(MAX_CLIENTS * MSG_SIZE),
        math::next_log2(MSG_SIZE),
    )
    .expect("Unable to create rgate");
    rg.activate().expect("Unable to activate rgate");
    RGATE.set(Some(rg));

    server_loop(|| {
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
