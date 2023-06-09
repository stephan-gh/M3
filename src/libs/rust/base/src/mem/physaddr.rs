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
use crate::goff;
use crate::impl_prim_int;
use crate::serialize::{Deserialize, Serialize};
use crate::tcu::EpId;

pub type PhysAddrRaw = u32;

#[derive(Default, Copy, Clone, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PhysAddr(PhysAddrRaw);

impl PhysAddr {
    pub const fn new(ep: EpId, off: PhysAddrRaw) -> Self {
        Self((ep as PhysAddrRaw) << 30 | (cfg::MEM_OFFSET as PhysAddrRaw) + off)
    }

    pub const fn new_raw(addr: PhysAddrRaw) -> Self {
        Self(addr)
    }

    pub const fn as_raw(&self) -> PhysAddrRaw {
        self.0
    }

    pub const fn as_goff(&self) -> goff {
        self.0 as goff
    }

    pub const fn ep(&self) -> EpId {
        ((self.0 - cfg::MEM_OFFSET as PhysAddrRaw) >> 30) as EpId
    }

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
