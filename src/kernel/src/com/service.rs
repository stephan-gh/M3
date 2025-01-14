/*
 * Copyright (C) 2020-2022 Nils Asmussen, Barkhausen Institut
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

use base::boxed::Box;
use base::cell::RefCell;
use base::col::String;
use base::errors::Error;
use base::mem::{MsgBuf, MsgBufRef};
use base::rc::{Rc, SRc, Weak};
use base::tcu;
use core::fmt;

use crate::cap::RGateObject;
use crate::com::{QueueId, SendQueue};
use crate::tiles::Activity;

pub struct Service {
    act: Weak<Activity>,
    name: String,
    rgate: SRc<RGateObject>,
    queue: RefCell<Box<SendQueue>>,
}

impl Service {
    pub fn new(act: &Rc<Activity>, name: String, rgate: SRc<RGateObject>) -> SRc<Self> {
        SRc::new(Service {
            act: Rc::downgrade(act),
            name,
            rgate,
            queue: RefCell::from(SendQueue::new(QueueId::Serv(act.id()), act.tile_id())),
        })
    }

    pub fn activity(&self) -> Rc<Activity> {
        self.act.upgrade().unwrap()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn send(&self, lbl: tcu::Label, msg: &MsgBuf) -> Result<thread::Event, Error> {
        let (_, rep) = self.rgate.location().unwrap();
        self.queue.borrow_mut().send(rep, lbl, msg)
    }

    pub fn send_receive_async(
        &self,
        lbl: tcu::Label,
        msg: MsgBufRef<'_>,
    ) -> Result<&'static tcu::Message, Error> {
        let event = self.send(lbl, &msg)?;
        drop(msg);
        SendQueue::receive_async(event)
    }

    pub fn abort(&self) {
        self.queue.borrow_mut().abort();
    }
}

impl fmt::Debug for Service {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Service[name={}, rgate=", self.name)?;
        self.rgate.print_loc(f)?;
        write!(f, "]")
    }
}
