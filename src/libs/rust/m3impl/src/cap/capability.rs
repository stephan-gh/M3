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

use bitflags::bitflags;
use core::ops;

use crate::cell::Cell;
use crate::kif;
use crate::syscalls;

/// A capability selector
pub type Selector = kif::CapSel;

bitflags! {
    /// Flags for [`Capability`]
    pub struct CapFlags : u32 {
        const KEEP_CAP   = 0x1;
    }
}

/// Represents a capability
#[derive(Debug)]
pub struct Capability {
    sel: Cell<Selector>,
    flags: CapFlags,
}

impl Capability {
    /// Creates a new `Capability` with given selector and flags.
    pub const fn new(sel: Selector, flags: CapFlags) -> Self {
        Capability {
            sel: Cell::new(sel),
            flags,
        }
    }

    /// Returns the selector.
    pub fn sel(&self) -> Selector {
        self.sel.get()
    }

    /// Returns the flags.
    pub fn flags(&self) -> CapFlags {
        self.flags
    }

    /// Sets the flags to `flags`.
    pub fn set_flags(&mut self, flags: CapFlags) {
        self.flags = flags;
    }

    fn release(&mut self) {
        if (self.flags & CapFlags::KEEP_CAP).is_empty() {
            let crd = kif::CapRngDesc::new(kif::CapType::OBJECT, self.sel(), 1);
            syscalls::revoke(kif::SEL_ACT, crd, true).ok();
        }
    }
}

impl ops::Drop for Capability {
    fn drop(&mut self) {
        self.release();
    }
}
