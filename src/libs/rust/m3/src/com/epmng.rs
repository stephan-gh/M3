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

use col::Vec;
use com::gate::Gate;
use com::{EPArgs, EP};
use errors::Error;
use kif::INVALID_SEL;
use syscalls;

/// The endpoint manager (`EpMng`) multiplexes all non-reserved endpoints among the gates.
pub struct EpMng {
    eps: Vec<EP>,
}

impl EpMng {
    pub fn new() -> Self {
        EpMng { eps: Vec::new() }
    }

    /// Allocates a new endpoint.
    pub fn acquire(&mut self, replies: u32) -> Result<EP, Error> {
        if replies > 0 {
            EP::new_with(EPArgs::new().replies(replies))
        }
        else if let Some(ep) = self.eps.pop() {
            Ok(ep)
        }
        else {
            EP::new()
        }
    }

    /// Frees the given endpoint
    pub fn release(&mut self, ep: EP, invalidate: bool) {
        if invalidate {
            syscalls::activate(ep.sel(), INVALID_SEL, 0).ok();
        }

        if ep.replies() == 0 {
            self.eps.push(ep);
        }
    }

    /// Allocates a new endpoint for the given gate and activates the gate. Returns the endpoint.
    pub(crate) fn activate(&mut self, gate: &Gate) -> Result<EP, Error> {
        let ep = self.acquire(0)?;
        syscalls::activate(ep.sel(), gate.sel(), 0).map(|_| ep)
    }

    pub(crate) fn reset(&mut self) {
        self.eps.clear();
    }
}