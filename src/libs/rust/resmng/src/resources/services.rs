/*
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

use m3::cap::{CapFlags, Capability, SelSpace, Selector};
use m3::col::{String, Vec};
use m3::com::SendGate;
use m3::errors::{Code, Error};
use m3::io::LogFlags;
use m3::log;
use m3::mem::MsgBuf;
use m3::serialize::M3Deserializer;
use m3::syscalls;
use m3::{build_vmsg, kif};

use core::cmp::Reverse;

use crate::childs;
use crate::events;
use crate::resources::Resources;
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
        log!(LogFlags::ResMngServ, "Creating service {}:{}", id, name);

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
        let dst = SelSpace::get().alloc_sels(2);
        let event = events::alloc_event();
        let id = self.id;
        let name = self.name.clone();
        syscalls::derive_srv(
            self.sel(),
            kif::CapRngDesc::new(kif::CapType::Object, dst, 2),
            sessions,
            event,
        )?;

        let reply = events::wait_for_async(child, event)?;
        let mut de = M3Deserializer::new(reply.as_words());
        de.skip(1);
        let reply = de.pop::<kif::upcalls::DeriveSrv>()?;
        Result::from(reply.error)?;

        Ok(Self::new(id, child, dst, dst + 1, name, sessions, false))
    }

    fn shutdown_async(&mut self) {
        log!(
            LogFlags::ResMngServ,
            "Sending SHUTDOWN to service {}:{}",
            self.id,
            self.name
        );

        let child = self.child;
        let mut smsg_buf = MsgBuf::borrow_def();
        build_vmsg!(smsg_buf, kif::service::Request::Shutdown);
        let event = self.queue.send(&smsg_buf);
        drop(smsg_buf);

        if let Ok(ev) = event {
            // ignore errors here
            events::wait_for_async(child, ev).ok();
        }
    }
}

pub struct Session {
    sel: Selector,
    ident: u64,
    serv: Id,
}

impl Session {
    pub fn new_async(
        child: childs::Id,
        sel: Selector,
        serv: &mut Service,
        arg: &str,
    ) -> Result<Self, Error> {
        let sid = serv.id;

        let mut smsg_buf = MsgBuf::borrow_def();
        build_vmsg!(smsg_buf, kif::service::Request::Open { arg });
        let event = serv.queue.send(&smsg_buf);
        drop(smsg_buf);

        event.and_then(|event| {
            let reply = events::wait_for_async(child, event)?;

            let mut de = M3Deserializer::new(reply.as_words());
            let res: Code = de.pop()?;
            if res != Code::Success {
                return Err(Error::new(res));
            }

            let reply: kif::service::OpenReply = de.pop()?;
            Ok(Session {
                sel,
                ident: reply.ident,
                serv: sid,
            })
        })
    }

    pub fn sel(&self) -> Selector {
        self.sel
    }

    pub fn ident(&self) -> u64 {
        self.ident
    }

    pub fn close_async(self, res: &mut Resources, child: childs::Id) -> Result<(), Error> {
        let event = {
            let serv = res.services_mut().get_mut_by_id(self.serv)?;

            let mut smsg_buf = MsgBuf::borrow_def();
            build_vmsg!(smsg_buf, kif::service::Request::Close { sid: self.ident });
            serv.queue.send(&smsg_buf)
        };

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

impl Default for ServiceManager {
    fn default() -> Self {
        Self {
            servs: Vec::new(),
            // start with 1, because we use that as a label in sendqueue and label 0 is special
            next_id: 1,
        }
    }
}

impl ServiceManager {
    pub fn get_with<P: FnMut(&&Service) -> bool>(&self, pred: P) -> Result<&Service, Error> {
        self.servs
            .iter()
            .find(pred)
            .ok_or_else(|| Error::new(Code::InvArgs))
    }

    pub fn get_mut_with<P: FnMut(&&mut Service) -> bool>(
        &mut self,
        pred: P,
    ) -> Result<&mut Service, Error> {
        self.servs
            .iter_mut()
            .find(pred)
            .ok_or_else(|| Error::new(Code::InvArgs))
    }

    pub fn get_mut_by_id(&mut self, id: Id) -> Result<&mut Service, Error> {
        self.get_mut_with(|s| s.id == id)
    }

    pub fn get_by_name(&self, name: &str) -> Result<&Service, Error> {
        self.get_with(|s| s.name == name)
    }

    pub fn get_mut_by_name(&mut self, name: &str) -> Result<&mut Service, Error> {
        self.get_mut_with(|s| s.name == name)
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
        if self.get_mut_by_name(&name).is_ok() {
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

    pub fn remove_service(&mut self, id: Id) -> Service {
        let idx = self.servs.iter().position(|s| s.id == id).unwrap();
        let serv = self.servs.remove(idx);

        log!(
            LogFlags::ResMngServ,
            "Removing service {}:{}",
            serv.id,
            serv.name
        );

        serv
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
            if let Ok(serv) = self.get_mut_by_id(id) {
                Service::shutdown_async(serv);
            }
        }
    }
}
