/*
 * Copyright (C) 2023 Nils Asmussen, Barkhausen Institut
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
use core::ops;

use crate::cfg;
use crate::impl_prim_int;
use crate::mem::GlobOff;
use crate::serialize::{Deserialize, Serialize};
use crate::tcu::EpId;

/// The underlying type for [`PhysAddr`]
pub type PhysAddrRaw = u32;

/// Represents a physical address
///
/// Physical addresses are used locally on a tile and need to first go through the TCU's physical
/// memory protection (PMP) to obtain the final address in memory. For that reason, physical
/// addresses consist of an endpoint id and an offset to refer to a specific offset in a memory
/// region accessed via a specific PMP endpoint.
#[derive(Default, Copy, Clone, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PhysAddr(PhysAddrRaw);

impl PhysAddr {
    /// Creates a new physical address for given endpoint and offset
    pub const fn new(ep: EpId, off: PhysAddrRaw) -> Self {
        Self((ep as PhysAddrRaw) << 30 | (cfg::MEM_OFFSET as PhysAddrRaw) + off)
    }

    /// Creates a new physical address from given raw address
    pub const fn new_raw(addr: PhysAddrRaw) -> Self {
        Self(addr)
    }

    /// Returns the underlying raw address
    pub const fn as_raw(&self) -> PhysAddrRaw {
        self.0
    }

    /// Returns this address as a global offset
    pub const fn as_goff(&self) -> GlobOff {
        self.0 as GlobOff
    }

    /// Returns the endpoint of this physical address
    pub const fn ep(&self) -> EpId {
        ((self.0 - cfg::MEM_OFFSET as PhysAddrRaw) >> 30) as EpId
    }

    /// Returns the offset of this physical address
    pub const fn offset(&self) -> PhysAddrRaw {
        (self.0 - cfg::MEM_OFFSET as PhysAddrRaw) & 0x3FFF_FFFF
    }
}

impl fmt::Display for PhysAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "P[EP{}+{:#x}]", self.ep(), self.offset())
    }
}

impl fmt::Debug for PhysAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "P[EP{}+{:#x}]", self.ep(), self.offset())
    }
}

impl ops::Add<PhysAddrRaw> for PhysAddr {
    type Output = Self;

    fn add(self, rhs: PhysAddrRaw) -> Self::Output {
        Self(self.0 + (rhs as PhysAddrRaw))
    }
}

impl ops::AddAssign<PhysAddrRaw> for PhysAddr {
    fn add_assign(&mut self, rhs: PhysAddrRaw) {
        self.0 += rhs as PhysAddrRaw;
    }
}

impl_prim_int!(PhysAddr, PhysAddrRaw);
