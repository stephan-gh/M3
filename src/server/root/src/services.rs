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

use core::intrinsics;
use m3::cap::{Capability, CapFlags, Selector};
use m3::cell::StaticCell;
use m3::col::{String, Vec};
use m3::com::{RecvGate, SendGate, SGateArgs};
use m3::errors::{Code, Error};
use m3::kif;
use m3::syscalls;
use m3::util;
use m3::vpe::VPE;
use thread;

use childs;
use childs::{Child, Id};
use sendqueue::SendQueue;

pub struct Service {
    id: Id,
    _cap: Capability,
    queue: SendQueue,
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
            queue: SendQueue::new(id, sgate),
            _rgate: rgate,
            name: name,
            child: child.id(),
        })
    }

    pub fn name(&self) -> &String {
        &self.name
    }
    pub fn queue(&mut self) -> &mut SendQueue {
        &mut self.queue
    }

    fn child(&mut self) -> &mut Child {
        childs::get().child_by_id_mut(self.child).unwrap()
    }

    fn shutdown(&mut self) {
        log!(ROOT, "Sending SHUTDOWN to service {}", self.name);

        let smsg = kif::service::Shutdown {
            opcode: kif::service::Operation::SHUTDOWN.val as u64,
        };
        let event = self.queue.send(util::object_to_bytes(&smsg));

        if let Ok(ev) = event {
            thread::ThreadManager::get().wait_for(ev);
        }
    }
}

pub struct Session {
    pub sel: Selector,
    pub ident: u64,
    pub serv: Id,
}

impl Session {
    pub fn new(sel: Selector, serv: &mut Service, arg: u64) -> Result<(Selector, Self), Error> {
        let smsg = kif::service::Open {
            opcode: kif::service::Operation::OPEN.val as u64,
            arg: arg,
        };
        let event = serv.queue.send(util::object_to_bytes(&smsg));

        event.and_then(|event| {
            thread::ThreadManager::get().wait_for(event);

            let reply = thread::ThreadManager::get().fetch_msg().ok_or(Error::new(Code::RecvGone))?;
            let reply: &[kif::service::OpenReply] = unsafe { intrinsics::transmute(&reply.data) };
            let reply = &reply[0];

            if reply.res != 0 {
                return Err(Error::from(reply.res as u32));
            }

            Ok((reply.sess as Selector,
                Session {
                sel: sel,
                ident: reply.ident,
                serv: serv.id,
            }))
        })
    }

    pub fn close(&self) -> Result<(), Error> {
        let serv = get().get_by_id(self.serv)?;

        let smsg = kif::service::Close {
            opcode: kif::service::Operation::CLOSE.val as u64,
            sess: self.ident,
        };
        let event = serv.queue.send(util::object_to_bytes(&smsg));

        event.map(|ev| {
            thread::ThreadManager::get().wait_for(ev)
        })
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
            // start with 1, because we use that as a label in sendqueue and label 0 is special
            next_id: 1,
        }
    }

    pub fn get(&mut self, name: &String) -> Result<&mut Service, Error> {
        self.servs.iter_mut().find(|s| s.name == *name).ok_or(Error::new(Code::InvArgs))
    }
    pub fn get_by_id(&mut self, id: Id) -> Result<&mut Service, Error> {
        self.servs.iter_mut().find(|s| s.id == id).ok_or(Error::new(Code::InvArgs))
    }
    pub fn remove_service(&mut self, id: Id) {
        let idx = self.servs.iter().position(|s| s.id == id).unwrap();
        self.servs[idx].queue.abort();
        self.servs.remove(idx);
    }

    pub fn reg_serv(&mut self, child: &mut Child, child_sel: Selector, dst_sel: Selector,
                    rgate_sel: Selector, name: String) -> Result<(), Error> {
        log!(ROOT, "{}: reg_serv(child_sel={}, dst_sel={}, rgate_sel={}, name={})",
             child.name(), child_sel, dst_sel, rgate_sel, name);

        if child.has_service(dst_sel) {
            return Err(Error::new(Code::InvArgs));
        }
        if self.get(&name).is_ok() {
            return Err(Error::new(Code::Exists));
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
            serv.shutdown();
        }
        self.remove_service(id);
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
        let (srv_sel, sess) = Session::new(dst_sel, serv, arg)?;

        let our_sel = serv.child().obtain(srv_sel)?;
        child.delegate(our_sel, dst_sel)?;
        child.add_session(sess);
        Ok(())
    }

    pub fn close_session(&mut self, child: &mut Child, sel: Selector) -> Result<(), Error> {
        log!(ROOT, "{}: close_sess(sel={})", child.name(), sel);

        let sess = child.remove_session(sel)?;
        sess.close()
    }

    pub fn shutdown(&mut self) {
        // first collect the ids
        let mut ids = Vec::new();
        for s in &self.servs {
            ids.push(s.id);
        }
        // reverse sort to shutdown the services in reverse order
        ids.sort_by(|a,b| b.cmp(a));

        // now send a shutdown request to all that still exist.
        // this is required, because shutdown switches the thread, so that the service list can
        // change in the meantime.
        for id in ids {
            if let Ok(serv) = self.get_by_id(id) {
                serv.shutdown();
            }
        }
    }
}
