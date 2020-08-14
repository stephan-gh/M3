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

use base::cell::RefCell;
use base::col::String;
use base::errors::{Code, Error};
use base::rc::{Rc, SRc, Weak};
use base::tcu;
use core::fmt;

use crate::cap::RGateObject;
use crate::com::SendQueue;
use crate::pes::VPE;

pub struct Service {
    vpe: Weak<VPE>,
    name: String,
    rgate: SRc<RGateObject>,
    queue: RefCell<SendQueue>,
}

impl Service {
    pub fn new(vpe: &Rc<VPE>, name: String, rgate: SRc<RGateObject>) -> SRc<Self> {
        SRc::new(Service {
            vpe: Rc::downgrade(vpe),
            name,
            rgate,
            queue: RefCell::from(SendQueue::new(vpe.id() as u64, vpe.pe_id())),
        })
    }

    pub fn vpe(&self) -> Rc<VPE> {
        self.vpe.upgrade().unwrap()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn send(&self, lbl: tcu::Label, msg: &[u8]) -> Result<thread::Event, Error> {
        let (_, rep) = self.rgate.location().unwrap();
        self.queue.borrow_mut().send(rep, lbl, msg)
    }

    pub fn send_receive(
        serv: &SRc<Service>,
        lbl: tcu::Label,
        msg: &[u8],
    ) -> Result<&'static tcu::Message, Error> {
        let event = serv.send(lbl, msg);

        event.and_then(|event| {
            thread::ThreadManager::get().wait_for(event);
            thread::ThreadManager::get()
                .fetch_msg()
                .ok_or_else(|| Error::new(Code::RecvGone))
        })
    }

    pub fn abort(&self) {
        self.queue.borrow_mut().abort();
    }
}

impl fmt::Debug for Service {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Service[name={}, rgate=", self.name)?;
        self.rgate.print_loc(f)?;
        write!(f, "]")
    }
}
