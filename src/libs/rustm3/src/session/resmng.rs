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

use cap::Selector;
use com::{RecvGate, SendGate};
use errors::Error;
use kif;

int_enum! {
    /// The resource manager calls
    pub struct ResMngOperation : u64 {
        const CLONE         = 0x0;
        const REG_SERV      = 0x1;
        const OPEN_SESS     = 0x2;
        const CLOSE_SESS    = 0x3;
    }
}

pub struct ResMng {
    sgate: SendGate,
}

impl ResMng {
    pub fn new(sgate: SendGate) -> Self {
        ResMng {
            sgate: sgate,
        }
    }

    pub fn sel(&self) -> Selector {
        self.sgate.sel()
    }

    // TODO temporary
    pub fn valid(&self) -> bool {
        self.sgate.sel() != kif::INVALID_SEL
    }

    pub fn clone(&self) -> Self {
        // TODO clone the send gate to the current rmng
        ResMng::new(SendGate::new_bind(kif::INVALID_SEL))
    }

    pub fn register_service(&self, dst: Selector, rgate: Selector, name: &str) -> Result<(), Error> {
        send_recv_res!(
            &self.sgate, RecvGate::def(),
            ResMngOperation::REG_SERV, dst, rgate, name
        ).map(|_| ())
    }

    pub fn open_sess(&self, dst: Selector, name: &str, arg: u64) -> Result<(), Error> {
        send_recv_res!(
            &self.sgate, RecvGate::def(),
            ResMngOperation::OPEN_SESS, dst, name, arg
        ).map(|_| ())
    }

    pub fn close_sess(&self, sel: Selector) -> Result<(), Error> {
        send_recv_res!(
            &self.sgate, RecvGate::def(),
            ResMngOperation::CLOSE_SESS, sel
        ).map(|_| ())
    }
}
