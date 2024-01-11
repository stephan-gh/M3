/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
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

use crate::cap::Selector;
use crate::cell::{RefMut, StaticRefCell};
use crate::col::Vec;
use crate::com::{EPArgs, EP};
use crate::errors::Error;
use crate::kif::INVALID_SEL;
use crate::syscalls;
use crate::tcu::EpId;

/// The endpoint manager (`EpMng`)
///
/// The `EpMng` is responsible for endpoint allocation and deallocation. It will also reuse already
/// allocated, but no longer used endpoints for new allocations, if possible.
pub struct EpMng {
    eps: Vec<EP>,
}

static EPMNG: StaticRefCell<EpMng> = StaticRefCell::new(EpMng { eps: Vec::new() });

impl EpMng {
    /// Returns the `EpMng` instance
    pub fn get() -> RefMut<'static, EpMng> {
        EPMNG.borrow_mut()
    }

    /// Allocates a specific endpoint for the given activity.
    pub fn acquire_for(act: Selector, ep: EpId, replies: usize) -> Result<EP, Error> {
        EP::new_with(EPArgs::default().epid(ep).activity(act).replies(replies))
    }

    /// Allocates a new endpoint.
    pub fn acquire(&mut self, replies: usize) -> Result<EP, Error> {
        if replies > 0 {
            EP::new_with(EPArgs::default().replies(replies))
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
        if ep.is_standard() {
            return;
        }

        if invalidate {
            syscalls::activate(ep.sel(), INVALID_SEL, INVALID_SEL, 0).ok();
        }

        if ep.is_cacheable() {
            self.eps.push(ep);
        }
    }

    /// Allocates a new endpoint for the given gate and activates the gate. Returns the endpoint.
    pub(crate) fn activate(&mut self, gate: Selector) -> Result<EP, Error> {
        let ep = self.acquire(0)?;
        syscalls::activate(ep.sel(), gate, INVALID_SEL, 0).map(|_| ep)
    }
}
