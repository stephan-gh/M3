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
use m3::cell::{Ref, RefMut, StaticRefCell};
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
        log!(crate::LOG_SERV, "Creating service {}:{}", id, name);

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

    pub fn derive_async(
        serv: Ref<'_, Self>,
        child: childs::Id,
        sessions: u32,
    ) -> Result<Self, Error> {
        let dst = VPE::cur().alloc_sels(2);
        let event = events::alloc_event();
        let id = serv.id;
        let name = serv.name.clone();
        syscalls::derive_srv(
            serv.sel(),
            kif::CapRngDesc::new(kif::CapType::OBJECT, dst, 2),
            sessions,
            event,
        )?;
        drop(serv);

        let reply = events::wait_for_async(child, event)?;
        let reply = reply.get_data::<kif::upcalls::DeriveSrv>();
        Result::from(Code::from(reply.error as u32))?;

        Ok(Self::new(id, child, dst, dst + 1, name, sessions, false))
    }

    fn shutdown_async(mut serv: RefMut<'_, Self>) {
        log!(
            crate::LOG_SERV,
            "Sending SHUTDOWN to service {}:{}",
            serv.id,
            serv.name
        );

        let child = serv.child;
        let mut smsg_buf = MsgBuf::borrow_def();
        smsg_buf.set(kif::service::Shutdown {
            opcode: kif::service::Operation::SHUTDOWN.val as u64,
        });
        let event = serv.queue.send(&smsg_buf);
        drop(smsg_buf);
        drop(serv);

        if let Ok(ev) = event {
            // ignore errors here
            events::wait_for_async(child, ev).ok();
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
        mut serv: RefMut<'_, Service>,
        arg: &str,
    ) -> Result<Self, Error> {
        let sid = serv.id;

        let mut smsg_buf = MsgBuf::borrow_def();
        smsg_buf.set(kif::service::Open::new(arg));
        let event = serv.queue.send(&smsg_buf);
        drop(smsg_buf);
        drop(serv);

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
                serv: sid,
            })
        })
    }

    pub fn sel(&self) -> Selector {
        self.sel
    }

    pub fn ident(&self) -> Label {
        self.ident
    }

    pub fn close_async(self, child: childs::Id) -> Result<(), Error> {
        let event = {
            let mut serv = get_mut_by_id(self.serv)?;

            let mut smsg_buf = MsgBuf::borrow_def();
            smsg_buf.set(kif::service::Close {
                opcode: kif::service::Operation::CLOSE.val as u64,
                sess: self.ident as u64,
            });
            serv.queue.send(&smsg_buf)
        };

        if let Ok(ev) = event {
            // ignore errors
            events::wait_for_async(child, ev).ok();
        }
        Ok(())
    }
}

struct ServiceManager {
    servs: Vec<Service>,
    next_id: Id,
}

static MNG: StaticRefCell<ServiceManager> = StaticRefCell::new(ServiceManager {
    servs: Vec::new(),
    // start with 1, because we use that as a label in sendqueue and label 0 is special
    next_id: 1,
});

fn mng() -> Ref<'static, ServiceManager> {
    MNG.borrow()
}

fn mng_mut() -> RefMut<'static, ServiceManager> {
    MNG.borrow_mut()
}

pub fn get_with<P: Fn(&Service) -> bool>(pred: P) -> Result<Ref<'static, Service>, Error> {
    let mng = mng();
    let idx = mng
        .servs
        .iter()
        .position(|s| pred(s))
        .ok_or_else(|| Error::new(Code::InvArgs))?;
    Ok(Ref::map(mng, |mng| &mng.servs[idx]))
}

pub fn get_mut_with<P: Fn(&Service) -> bool>(pred: P) -> Result<RefMut<'static, Service>, Error> {
    let mng = mng_mut();
    let idx = mng
        .servs
        .iter()
        .position(|s| pred(s))
        .ok_or_else(|| Error::new(Code::InvArgs))?;
    Ok(RefMut::map(mng, |mng| &mut mng.servs[idx]))
}

pub fn get_by_id(id: Id) -> Result<Ref<'static, Service>, Error> {
    get_with(|s| s.id == id)
}

pub fn get_mut_by_id(id: Id) -> Result<RefMut<'static, Service>, Error> {
    get_mut_with(|s| s.id == id)
}

pub fn get_by_name(name: &str) -> Result<Ref<'static, Service>, Error> {
    get_with(|s| s.name == name)
}

pub fn get_mut_by_name(name: &str) -> Result<RefMut<'static, Service>, Error> {
    get_mut_with(|s| s.name == name)
}

pub fn add_service(
    child: childs::Id,
    srv_sel: Selector,
    sgate_sel: Selector,
    name: String,
    sessions: u32,
    owned: bool,
) -> Result<Id, Error> {
    if get_mut_by_name(&name).is_ok() {
        return Err(Error::new(Code::Exists));
    }

    let mut mng = mng_mut();
    let serv = Service::new(
        mng.next_id,
        child,
        srv_sel,
        sgate_sel,
        name,
        sessions,
        owned,
    );
    mng.servs.push(serv);
    mng.next_id += 1;

    Ok(mng.next_id - 1)
}

pub fn remove_service_async(id: Id, notify: bool) -> Service {
    let idx = {
        let mng = mng_mut();
        let idx = mng.servs.iter().position(|s| s.id == id).unwrap();
        let serv = RefMut::map(mng, |mng| &mut mng.servs[idx]);

        log!(
            crate::LOG_SERV,
            "Removing service {}:{}",
            serv.id,
            serv.name
        );

        if notify {
            // we need to do that before we remove the service
            Service::shutdown_async(serv);
        }
        idx
    };

    mng_mut().servs.remove(idx)
}

pub fn shutdown_async() {
    // first collect the ids
    let mut ids = Vec::new();
    for s in &mng().servs {
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
        if let Ok(serv) = get_mut_by_id(id) {
            Service::shutdown_async(serv);
        }
    }
}
