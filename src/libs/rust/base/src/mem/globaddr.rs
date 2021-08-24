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

use cfg_if::cfg_if;
use core::fmt;
use core::ops;

use crate::arch::tcu::PEId;
use crate::goff;

/// Represents a global address, which is a combination of a PE id and an offset within the PE.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct GlobAddr {
    val: u64,
}

cfg_if! {
    if #[cfg(not(target_vendor = "host"))] {
        const PE_SHIFT: u64 = 56;
        const PE_OFFSET: u64 = 0x80;
    }
    else {
        const PE_SHIFT: u64 = 48;
        const PE_OFFSET: u64 = 0x0;
    }
}

impl GlobAddr {
    /// Creates a new global address from the given raw value
    pub fn new(addr: u64) -> GlobAddr {
        GlobAddr { val: addr }
    }

    /// Creates a new global address from the given PE id and offset
    pub fn new_with(pe: PEId, off: goff) -> GlobAddr {
        Self::new(((0x80 + pe as u64) << PE_SHIFT) | off)
    }

    /// Returns the raw value
    pub fn raw(self) -> u64 {
        self.val
    }

    /// Returns whether a PE id is set
    pub fn has_pe(self) -> bool {
        self.val >= (PE_OFFSET << PE_SHIFT)
    }

    /// Returns the PE id
    pub fn pe(self) -> PEId {
        ((self.val >> PE_SHIFT) - 0x80) as PEId
    }

    /// Returns the offset
    pub fn offset(self) -> goff {
        (self.val & ((1 << PE_SHIFT) - 1)) as goff
    }
}

impl fmt::Debug for GlobAddr {
    #[allow(clippy::absurd_extreme_comparisons)]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.has_pe() {
            write!(f, "G[PE{}+{:#x}]", self.pe(), self.offset())
        }
        // we need global addresses without PE prefix for, e.g., the TCU MMIO region
        else {
            write!(f, "G[{:#x}]", self.raw())
        }
    }
}

impl ops::Add<goff> for GlobAddr {
    type Output = GlobAddr;

    fn add(self, rhs: goff) -> Self::Output {
        GlobAddr::new(self.val + rhs)
    }
}
