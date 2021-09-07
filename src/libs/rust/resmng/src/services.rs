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

use m3::cap::{CapFlags, Capability, Selector};
use m3::cell::StaticCell;
use m3::col::{String, Vec};
use m3::com::SendGate;
use m3::errors::{Code, Error};
use m3::kif;
use m3::log;
use m3::mem::MsgBuf;
use m3::pes::VPE;
use m3::syscalls;
use m3::tcu::Label;

use core::cmp::Reverse;

use crate::childs;
use crate::events;
use crate::sendqueue::SendQueue;

pub type Id = u32;

pub struct Service {
    id: Id,
    child: childs::Id,
    cap: Capability,
    queue: SendQueue,
    name: String,
    sessions: u32,
    owned: bool,
}

impl Service {
    pub fn new(
        id: Id,
        child: childs::Id,
        srv_sel: Selector,
        sgate_sel: Selector,
        name: String,
        sessions: u32,
        owned: bool,
    ) -> Self {
        Service {
            id,
            child,
            cap: Capability::new(srv_sel, CapFlags::empty()),
            queue: SendQueue::new(id, SendGate::new_bind(sgate_sel)),
            name,
            sessions,
            owned,
        }
    }

    pub fn sel(&self) -> Selector {
        self.cap.sel()
    }

    pub fn sgate_sel(&self) -> Selector {
        self.queue.sgate_sel()
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn queue(&mut self) -> &mut SendQueue {
        &mut self.queue
    }

    pub fn sessions(&self) -> u32 {
        self.sessions
    }

    pub fn derive_async(&self, child: childs::Id, sessions: u32) -> Result<Self, Error> {
        let dst = VPE::cur().alloc_sels(2);
        let event = events::alloc_event();
        syscalls::derive_srv(
            self.sel(),
            kif::CapRngDesc::new(kif::CapType::OBJECT, dst, 2),
            sessions,
            event,
        )?;

        let reply = events::wait_for_async(child, event)?;
        let reply = reply.get_data::<kif::upcalls::DeriveSrv>();
        Result::from(Code::from(reply.error as u32))?;

        Ok(Self::new(
            self.id,
            child,
            dst,
            dst + 1,
            self.name.clone(),
            sessions,
            false,
        ))
    }

    fn shutdown_async(&mut self) {
        log!(
            crate::LOG_SERV,
            "Sending SHUTDOWN to service '{}'",
            self.name
        );

        let mut smsg_buf = MsgBuf::borrow_def();
        smsg_buf.set(kif::service::Shutdown {
            opcode: kif::service::Operation::SHUTDOWN.val as u64,
        });
        let event = self.queue.send(&smsg_buf);
        drop(smsg_buf);

        if let Ok(ev) = event {
            // ignore errors here
            events::wait_for_async(self.child, ev).ok();
        }
    }
}

pub struct Session {
    sel: Selector,
    ident: Label,
    serv: Id,
}

impl Session {
    pub fn new_async(
        child: childs::Id,
        sel: Selector,
        serv: &mut Service,
        arg: &str,
    ) -> Result<Self, Error> {
        let mut smsg_buf = MsgBuf::borrow_def();
        smsg_buf.set(kif::service::Open::new(arg));
        let event = serv.queue.send(&smsg_buf);
        drop(smsg_buf);

        event.and_then(|event| {
            let reply = events::wait_for_async(child, event)?;
            let reply = reply.get_data::<kif::service::OpenReply>();

            let res = Code::from(reply.res as u32);
            if res != Code::None {
                return Err(Error::new(res));
            }

            Ok(Session {
                sel,
                ident: reply.ident as Label,
                serv: serv.id,
            })
        })
    }

    pub fn sel(&self) -> Selector {
        self.sel
    }

    pub fn ident(&self) -> Label {
        self.ident
    }

    pub fn close_async(&self, child: childs::Id) -> Result<(), Error> {
        let serv = get().get_by_id(self.serv)?;

        let mut smsg_buf = MsgBuf::borrow_def();
        smsg_buf.set(kif::service::Close {
            opcode: kif::service::Operation::CLOSE.val as u64,
            sess: self.ident as u64,
        });
        let event = serv.queue.send(&smsg_buf);
        drop(smsg_buf);

        if let Ok(ev) = event {
            // ignore errors
            events::wait_for_async(child, ev).ok();
        }
        Ok(())
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

    pub fn get(&mut self, name: &str) -> Result<&mut Service, Error> {
        self.servs
            .iter_mut()
            .find(|s| s.name == *name)
            .ok_or_else(|| Error::new(Code::InvArgs))
    }

    pub fn get_by_id(&mut self, id: Id) -> Result<&mut Service, Error> {
        self.servs
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| Error::new(Code::InvArgs))
    }

    pub fn add_service(
        &mut self,
        child: childs::Id,
        srv_sel: Selector,
        sgate_sel: Selector,
        name: String,
        sessions: u32,
        owned: bool,
    ) -> Result<Id, Error> {
        if self.get(&name).is_ok() {
            return Err(Error::new(Code::Exists));
        }

        let serv = Service::new(
            self.next_id,
            child,
            srv_sel,
            sgate_sel,
            name,
            sessions,
            owned,
        );
        self.servs.push(serv);
        self.next_id += 1;

        Ok(self.next_id - 1)
    }

    pub fn remove_service_async(&mut self, id: Id, notify: bool) -> Service {
        let idx = self.servs.iter().position(|s| s.id == id).unwrap();

        log!(
            crate::LOG_SERV,
            "Removing service {}",
            self.get_by_id(id).unwrap().name
        );

        if notify {
            // we need to do that before we remove the service
            let serv = self.get_by_id(id).unwrap();
            serv.shutdown_async();
        }

        self.servs.remove(idx)
    }

    pub fn shutdown_async(&mut self) {
        // first collect the ids
        let mut ids = Vec::new();
        for s in &self.servs {
            if s.owned {
                ids.push(s.id);
            }
        }
        // reverse sort to shutdown the services in reverse order
        ids.sort_by_key(|&b| Reverse(b));

        // now send a shutdown request to all that still exist.
        // this is required, because shutdown switches the thread, so that the service list can
        // change in the meantime.
        for id in ids {
            if let Ok(serv) = self.get_by_id(id) {
                serv.shutdown_async();
            }
        }
    }
}
