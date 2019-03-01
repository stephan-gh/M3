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

use m3::cap::{Capability, CapFlags, Selector};
use m3::cell::StaticCell;
use m3::col::{String, Vec};
use m3::com::{RecvGate, recv_res, SendGate, SGateArgs};
use m3::errors::{Code, Error};
use m3::kif;
use m3::syscalls;
use m3::util;
use m3::vpe::VPE;

use childs;
use childs::{Child, Id};

pub struct Service {
    _cap: Capability,
    sgate: SendGate,
    _rgate: RecvGate,
    name: String,
    child: Id,
}

impl Service {
    pub fn new(child: &mut Child, dst_sel: Selector,
               rgate_sel: Selector, name: String) -> Result<Self, Error> {
        let sel = VPE::cur().alloc_sel();
        let rgate = RecvGate::new_bind(child.obtain(rgate_sel)?, util::next_log2(512));
        let sgate = SendGate::new_with(SGateArgs::new(&rgate).credits(256))?;
        syscalls::create_srv(sel, child.vpe_sel(), rgate.sel(), &name)?;
        child.delegate(sel, dst_sel)?;

        Ok(Service {
            _cap: Capability::new(sel, CapFlags::empty()),
            sgate: sgate,
            _rgate: rgate,
            name: name,
            child: child.id(),
        })
    }

    fn child(&mut self) -> &mut Child {
        childs::get().child_by_id_mut(self.child).unwrap()
    }
}

pub struct ServiceManager {
    servs: Vec<Service>,
}

static MNG: StaticCell<ServiceManager> = StaticCell::new(ServiceManager::new());

pub fn get() -> &'static mut ServiceManager {
    MNG.get_mut()
}

impl ServiceManager {
    pub const fn new() -> Self {
        ServiceManager {
            servs: Vec::new(),
        }
    }

    pub fn get(&mut self, name: &String) -> Result<&mut Service, Error> {
        self.servs.iter_mut().find(|s| s.name == *name).ok_or(Error::new(Code::InvArgs))
    }

    pub fn register(&mut self, child: &mut Child, dst_sel: Selector,
                    rgate_sel: Selector, name: String) -> Result<(), Error> {
        log!(ROOT, "{}: reg_serv(dst_sel={}, rgate_sel={}, name={})",
             child.name(), dst_sel, rgate_sel, name);

        let serv = Service::new(child, dst_sel, rgate_sel, name)?;
        self.servs.push(serv);
        Ok(())
    }

    pub fn open_session(&mut self, child: &mut Child, dst_sel: Selector,
                        name: String, arg: u64) -> Result<(), Error> {
        log!(ROOT, "{}: open_sess(dst_sel={}, name={}, arg={})",
             child.name(), dst_sel, name, arg);

        if child.get_session(dst_sel).is_some() {
            return Err(Error::new(Code::InvArgs));
        }

        let serv = self.get(&name)?;

        // TODO do that asynchronously
        let smsg = kif::service::Open {
            opcode: kif::service::Operation::OPEN.val as u64,
            arg: arg,
        };

        serv.sgate.send(&[smsg], RecvGate::def())?;
        let mut sis = recv_res(RecvGate::def())?;
        let srv_sel: Selector = sis.pop();
        let ident: u64 = sis.pop();

        let our_sel = serv.child().obtain(srv_sel)?;
        child.delegate(our_sel, dst_sel)?;
        child.add_session(dst_sel, ident, name);
        Ok(())
    }

    pub fn close_session(&mut self, child: &mut Child, sel: Selector) -> Result<(), Error> {
        log!(ROOT, "{}: close_sess(sel={})", child.name(), sel);

        {
            let sess = child.get_session(sel).ok_or(Error::new(Code::InvArgs))?;
            let serv = self.get(&sess.serv)?;

            // TODO do that asynchronously
            let smsg = kif::service::Close {
                opcode: kif::service::Operation::CLOSE.val as u64,
                sess: sess.ident,
            };

            serv.sgate.send(&[smsg], RecvGate::def())?;
            recv_res(RecvGate::def())?;
        }

        child.remove_session(sel);
        Ok(())
    }

    pub fn shutdown(&mut self) {
        for s in &self.servs {
            log!(ROOT, "Sending SHUTDOWN to service {}", s.name);

            // TODO do that asynchronously
            let smsg = kif::service::Shutdown {
                opcode: kif::service::Operation::SHUTDOWN.val as u64,
            };

            s.sgate.send(&[smsg], RecvGate::def()).ok();
            recv_res(RecvGate::def()).ok();
        }
    }
}
