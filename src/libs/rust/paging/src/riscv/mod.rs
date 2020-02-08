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

use base::cfg;
use base::kif::PageFlags;

pub type MMUPTE = u64;

pub const PTE_BITS: usize = 3;
pub const PTE_REC_IDX: usize = 0x1;

pub const LEVEL_CNT: usize = 3;
pub const LEVEL_BITS: usize = cfg::PAGE_BITS - PTE_BITS;
pub const LEVEL_MASK: usize = (1 << LEVEL_BITS) - 1;

bitflags! {
    pub struct MMUFlags : MMUPTE {
        const P     = 0b0000_0001;          // present
        const U     = 0b0100_0000;          // user accessible
        const NW    = 0b1000_0000;          // non-writable
        const NX    = 1 << 54 | 1 << 53;    // never-execute and privileged never-execute
        const NG    = 1 << 11;              // non-global
        const A     = 1 << 10;              // accessed

        const TYPE  = 0b11;
        const TBL   = 0b11;
        const BLK   = 0b01;
        const PAGE  = 0b11;

        const RW    = Self::A.bits | Self::P.bits | Self::NX.bits;
        const RWX   = Self::A.bits | Self::P.bits;

        const FLAGS = cfg::PAGE_MASK as u64 | Self::NX.bits;
    }
}

impl MMUFlags {
    pub fn table_flags() -> Self {
        unimplemented!();
    }

    pub fn page_flags() -> Self {
        unimplemented!();
    }

    pub fn lpage_flags() -> Self {
        unimplemented!();
    }

    pub fn is_lpage(&self) -> bool {
        unimplemented!();
    }

    pub fn perms_missing(&self, _perms: Self) -> bool {
        unimplemented!();
    }
}

pub fn needs_invalidate(_new_flags: MMUFlags, _old_flags: MMUFlags) -> bool {
    unimplemented!();
}

#[no_mangle]
pub extern "C" fn to_page_flags(_pte: MMUFlags) -> PageFlags {
    unimplemented!();
}

pub fn to_mmu_perms(_flags: PageFlags) -> MMUFlags {
    unimplemented!();
}

#[no_mangle]
pub extern "C" fn enable_paging() {
    unimplemented!();
}

pub fn invalidate_page(_id: u64, _virt: usize) {
    unimplemented!();
}

pub fn invalidate_tlb() {
    // TODO unimplemented!();
}

pub fn get_root_pt() -> MMUPTE {
    unimplemented!();
}

pub fn set_root_pt(_id: u64, _root: MMUPTE) {
    unimplemented!();
}

#[no_mangle]
pub extern "C" fn noc_to_phys(noc: u64) -> u64 {
    (noc & !0xFF00000000000000) | ((noc & 0xFF00000000000000) >> 8)
}

#[no_mangle]
pub extern "C" fn phys_to_noc(phys: u64) -> u64 {
    (phys & !0x00FF_0000_0000_0000) | ((phys & 0x00FF_0000_0000_0000) << 8)
}

pub fn get_pte_addr(_virt: usize, _level: usize) -> usize {
    unimplemented!();
}
