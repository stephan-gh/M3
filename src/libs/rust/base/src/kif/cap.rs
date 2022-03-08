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

use core::fmt;

/// A capability selector
pub type CapSel = u64;

/// A capability range descriptor, which describes a continuous range of capabilities
#[derive(Copy, Clone, Default)]
pub struct CapRngDesc {
    start: u64,
    count: u64,
}

int_enum! {
    /// The capability types
    pub struct CapType : u64 {
        /// Object capabilities are used for kernel objects (SendGate, Activity, ...)
        const OBJECT        = 0x0;
        /// Mapping capabilities are used for page table entries
        const MAPPING       = 0x1;
    }
}

impl CapRngDesc {
    /// Creates a new capability range descriptor. `start` is the first capability selector and
    /// `start + count - 1` is the last one.
    pub fn new(ty: CapType, start: CapSel, count: CapSel) -> CapRngDesc {
        CapRngDesc {
            start,
            count: count << 1 | ty.val,
        }
    }

    /// Creates a new capability range descriptor from the given raw value
    pub fn new_from(raw: [u64; 2]) -> CapRngDesc {
        CapRngDesc {
            start: raw[0],
            count: raw[1],
        }
    }

    /// Returns the raw value
    pub fn raw(self) -> [u64; 2] {
        [self.start, self.count]
    }

    /// Returns the capability type
    pub fn cap_type(self) -> CapType {
        CapType::from(self.count & 0x1)
    }

    /// Returns the first capability selector
    pub fn start(self) -> CapSel {
        self.start
    }

    /// Returns the number of capability selectors
    pub fn count(self) -> CapSel {
        self.count >> 1
    }
}

impl fmt::Display for CapRngDesc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CRD[{}: {}:{}]",
            self.cap_type(),
            self.start(),
            self.count()
        )
    }
}
