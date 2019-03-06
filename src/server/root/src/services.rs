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
    id: Id,
    _cap: Capability,
    sgate: SendGate,
    _rgate: RecvGate,
    name: String,
    child: Id,
}

impl Service {
    pub fn new(id: Id, child: &mut Child, dst_sel: Selector,
               rgate_sel: Selector, name: String) -> Result<Self, Error> {
        let sel = VPE::cur().alloc_sel();
        let rgate = RecvGate::new_bind(child.obtain(rgate_sel)?, util::next_log2(512));
        let sgate = SendGate::new_with(SGateArgs::new(&rgate).credits(256))?;
        syscalls::create_srv(sel, child.vpe_sel(), rgate.sel(), &name)?;
        child.delegate(sel, dst_sel)?;

        Ok(Service {
            id: id,
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
    next_id: Id,
}

static MNG: StaticCell<ServiceManager> = StaticCell::new(ServiceManager::new());

pub fn get() -> &'static mut ServiceManager {
    MNG.get_mut()
}

impl ServiceManager {
    pub const fn new() -> Self {
        ServiceManager {
            servs: Vec::new(),
            next_id: 0,
        }
    }

    pub fn get(&mut self, name: &String) -> Result<&mut Service, Error> {
        self.servs.iter_mut().find(|s| s.name == *name).ok_or(Error::new(Code::InvArgs))
    }
    pub fn get_by_id(&mut self, id: Id) -> Result<&mut Service, Error> {
        self.servs.iter_mut().find(|s| s.id == id).ok_or(Error::new(Code::InvArgs))
    }

    pub fn reg_serv(&mut self, child: &mut Child, child_sel: Selector, dst_sel: Selector,
                    rgate_sel: Selector, name: String) -> Result<(), Error> {
        log!(ROOT, "{}: reg_serv(child_sel={}, dst_sel={}, rgate_sel={}, name={})",
             child.name(), child_sel, dst_sel, rgate_sel, name);

        if child.has_service(dst_sel) {
            return Err(Error::new(Code::InvArgs));
        }

        let serv = if child_sel == 0 {
            Service::new(self.next_id, child, dst_sel, rgate_sel, name)
        }
        else {
            let server = child.child_mut(child_sel).ok_or(Error::new(Code::InvArgs))?;
            Service::new(self.next_id, server, dst_sel, rgate_sel, name)
        }?;

        child.add_service(serv.id, dst_sel);
        self.servs.push(serv);
        self.next_id += 1;
        Ok(())
    }

    pub fn unreg_serv(&mut self, child: &mut Child, sel: Selector, notify: bool) -> Result<(), Error> {
        log!(ROOT, "{}: unreg_serv(sel={})", child.name(), sel);

        let id = child.remove_service(sel)?;
        if notify {
            let serv = self.get_by_id(id).unwrap();
            Self::do_shutdown(serv);
        }
        self.servs.retain(|s| s.id != id);
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
            Self::do_shutdown(s);
        }
    }

    fn do_shutdown(serv: &Service) {
        log!(ROOT, "Sending SHUTDOWN to service {}", serv.name);

        // TODO do that asynchronously
        let smsg = kif::service::Shutdown {
            opcode: kif::service::Operation::SHUTDOWN.val as u64,
        };

        serv.sgate.send(&[smsg], RecvGate::def()).ok();
        recv_res(RecvGate::def()).ok();
    }
}
