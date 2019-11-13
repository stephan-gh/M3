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

use base::col::String;
use base::cell::RefCell;
use base::rc::Rc;
use base::errors::{Code, Error};
use base::tcu;
use core::fmt;

use com::SendQueue;
use pes::VPE;
use cap::RGateObject;

pub struct Service {
    vpe: Rc<RefCell<VPE>>,
    name: String,
    rgate: Rc<RefCell<RGateObject>>,
    queue: SendQueue,
}

impl Service {
    pub fn new(
        vpe: &Rc<RefCell<VPE>>,
        name: String,
        rgate: Rc<RefCell<RGateObject>>,
    ) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Service {
            vpe: vpe.clone(),
            name,
            rgate: rgate.clone(),
            queue: SendQueue::new(vpe.borrow().id() as u64, vpe.borrow().pe_id()),
        }))
    }

    pub fn vpe(&self) -> &Rc<RefCell<VPE>> {
        &self.vpe
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn send(&mut self, lbl: tcu::Label, msg: &[u8]) -> Result<thread::Event, Error> {
        let rep = self.rgate.borrow().ep().unwrap();
        self.queue.send(rep, lbl, msg)
    }

    pub fn send_receive(
        serv: &Rc<RefCell<Service>>,
        lbl: tcu::Label,
        msg: &[u8],
    ) -> Result<&'static tcu::Message, Error> {
        let event = serv.borrow_mut().send(lbl, msg);

        event.and_then(|event| {
            thread::ThreadManager::get().wait_for(event);
            thread::ThreadManager::get()
                .fetch_msg()
                .ok_or(Error::new(Code::RecvGone))
        })
    }

    pub fn abort(&mut self) {
        self.queue.abort();
    }
}

impl fmt::Debug for Service {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Service[name={}, rgate=", self.name)?;
        self.rgate.borrow().print_loc(f)?;
        write!(f, "]")
    }
}
