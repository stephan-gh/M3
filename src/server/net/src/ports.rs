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

use core::fmt;

use m3::cell::LazyStaticCell;
use m3::col::BitVec;
use m3::log;
use m3::net::Port;

static PORTS: LazyStaticCell<BitVec> = LazyStaticCell::default();

// ephemeral port range is from 49152 to 65535
const FIRST_PORT: Port = 49152;

pub struct EphemeralPort {
    port: Port,
}

impl EphemeralPort {
    fn new(port: Port) -> Self {
        log!(crate::LOG_PORTS, "ephemeral-ports: allocated {}", port);
        Self { port }
    }
}

impl Drop for EphemeralPort {
    fn drop(&mut self) {
        log!(crate::LOG_PORTS, "ephemeral-ports: freeing {}", self.port);
        free(self.port);
    }
}

impl core::ops::Deref for EphemeralPort {
    type Target = Port;

    fn deref(&self) -> &Self::Target {
        &self.port
    }
}

impl fmt::Display for EphemeralPort {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{}", self.port)
    }
}

pub fn init(sockets: usize) {
    PORTS.set(BitVec::new(sockets));
}

pub fn alloc() -> EphemeralPort {
    let idx = PORTS.first_clear();
    PORTS.get_mut().set(idx);
    EphemeralPort::new(FIRST_PORT + idx as Port)
}

pub fn is_ephemeral(port: Port) -> bool {
    port >= FIRST_PORT
}

fn free(port: Port) {
    let idx = (port - FIRST_PORT) as usize;
    PORTS.get_mut().clear(idx);
}
