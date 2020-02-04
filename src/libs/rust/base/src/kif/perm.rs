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

bitflags! {
    /// The permission bitmap that is used for memory and mapping capabilities.
    pub struct Perm : u32 {
        /// Read permission
        const R = 1;
        /// Write permission
        const W = 2;
        /// Execute permission
        const X = 4;
        /// Read + write permission
        const RW = Self::R.bits | Self::W.bits;
        /// Read + write + execute permission
        const RWX = Self::R.bits | Self::W.bits | Self::X.bits;
    }
}

/// A page table entry, containing a NoC address and PageFlags in the lower 4 bits
pub type PTE = u64;

bitflags! {
    /// The flags for virtual mappings
    pub struct PageFlags : u64 {
        /// Readable
        const R             = 0b0000_0001;
        /// Writable
        const W             = 0b0000_0010;
        /// Executable
        const X             = 0b0000_0100;
        /// User accessible
        const U             = 0b0000_1000;
        /// Large page
        const L             = 0b0001_0000;
        /// Read+write
        const RW            = Self::R.bits | Self::W.bits;
        /// Read+write+execute
        const RWX           = Self::R.bits | Self::W.bits | Self::X.bits;
        /// Read+execute
        const RX            = Self::R.bits | Self::X.bits;
        /// User+read+write+execute
        const IRWX          = Self::R.bits | Self::W.bits | Self::X.bits | Self::U.bits;
    }
}
