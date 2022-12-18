/*
 * Copyright (C) 2021 Nils Asmussen, Barkhausen Institut
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

use m3::cell::{LazyStaticRefCell, StaticCell};
use m3::col::BitArray;
use m3::log;
use m3::net::Port;

static PORTS: LazyStaticRefCell<BitArray> = LazyStaticRefCell::default();
static NEXT_PORT: StaticCell<Port> = StaticCell::new(0);

// ephemeral port range is from 49152 to 65535
const FIRST_PORT: Port = 49152;

pub enum AnyPort {
    Ephemeral(EphemeralPort),
    Manual(Port),
}

impl AnyPort {
    pub fn number(&self) -> Port {
        match self {
            Self::Ephemeral(e) => e.port,
            Self::Manual(p) => *p,
        }
    }
}

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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{}", self.port)
    }
}

pub fn init(sockets: usize) {
    PORTS.set(BitArray::new(sockets));
}

pub fn alloc() -> EphemeralPort {
    let mut ports = PORTS.borrow_mut();

    // don't reuse ports immediately, but cycle through them
    let next = NEXT_PORT.get();
    let res = if !ports.is_set(next as usize) {
        ports.set(next as usize);
        EphemeralPort::new(FIRST_PORT + next)
    }
    else {
        // if the next one isn't free, take the first free port
        let idx = ports.first_clear();
        ports.set(idx);
        EphemeralPort::new(FIRST_PORT + idx as Port)
    };

    // move NEXT_PORT forward
    NEXT_PORT.set((res.port - FIRST_PORT + 1) % (ports.size() as Port));

    res
}

pub fn is_ephemeral(port: Port) -> bool {
    port >= FIRST_PORT
}

fn free(port: Port) {
    let idx = (port - FIRST_PORT) as usize;
    PORTS.borrow_mut().clear(idx);
}
