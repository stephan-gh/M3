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

pub type MMUPTE = usize;

pub const PTE_BITS: usize = 3;
pub const PTE_REC_IDX: usize = 0x10;

pub const LEVEL_CNT: usize = 4;
pub const LEVEL_BITS: usize = cfg::PAGE_BITS - PTE_BITS;
pub const LEVEL_MASK: usize = (1 << LEVEL_BITS) - 1;

bitflags! {
    pub struct MMUFlags : MMUPTE {
        const P     = 0b0000_0001;
        const W     = 0b0000_0010;
        const U     = 0b0000_0100;
        const L     = 0b1000_0000;
        const NX    = 0x8000_0000_0000_0000;

        const RW    = Self::P.bits | Self::W.bits | Self::NX.bits;
        const RWX   = Self::P.bits | Self::W.bits;

        const FLAGS = cfg::PAGE_MASK | Self::NX.bits;
    }
}

impl MMUFlags {
    pub fn table_flags() -> Self {
        Self::RWX | Self::U
    }

    pub fn page_flags() -> Self {
        Self::empty()
    }

    pub fn lpage_flags() -> Self {
        Self::L
    }

    pub fn is_lpage(&self) -> bool {
        self.contains(Self::L)
    }

    pub fn perms_missing(&self, perms: Self) -> bool {
        !self.contains(Self::P)
            || (!self.contains(Self::W) && perms.contains(Self::W))
            || (self.contains(Self::NX) && !perms.contains(Self::NX))
    }
}

pub fn needs_invalidate(new_flags: MMUFlags, old_flags: MMUFlags) -> bool {
    old_flags.bits() != 0 && new_flags.perms_missing(old_flags)
}

#[no_mangle]
pub extern "C" fn to_page_flags(pte: MMUFlags) -> PageFlags {
    let mut res = PageFlags::empty();
    if pte.contains(MMUFlags::P) {
        res |= PageFlags::R;
    }
    if pte.contains(MMUFlags::W) {
        res |= PageFlags::W;
    }
    if pte.contains(MMUFlags::U) {
        res |= PageFlags::U;
    }
    if !pte.contains(MMUFlags::NX) {
        res |= PageFlags::X;
    }
    res
}

pub fn to_mmu_flags(flags: PageFlags) -> MMUFlags {
    let mut res = MMUFlags::empty();
    if flags.intersects(PageFlags::RWX) {
        res |= MMUFlags::P;
    }
    if flags.contains(PageFlags::W) {
        res |= MMUFlags::W;
    }
    if flags.contains(PageFlags::U) {
        res |= MMUFlags::U;
    }
    if !flags.contains(PageFlags::X) {
        res |= MMUFlags::NX;
    }
    res
}

#[no_mangle]
pub extern "C" fn enable_paging() {
    // already enabled by gem5
}

pub fn invalidate_page(_id: u64, virt: usize) {
    unsafe {
        asm!(
            "invlpg ($0)"
            : : "r"(virt)
            : : "volatile"
        );
    }
}

pub fn invalidate_tlb() {
    // nothing to do
}

pub fn get_root_pt() -> MMUPTE {
    let addr: MMUPTE;
    unsafe {
        asm!(
            "mov %cr3, $0" : "=r"(addr)
        )
    };
    addr
}

pub fn set_root_pt(_id: u64, root: MMUPTE) {
    unsafe {
        asm!(
            "mov $0, %cr3"
            : : "r"(root)
            : : "volatile"
        );
    }
}

#[no_mangle]
pub extern "C" fn noc_to_phys(noc: u64) -> u64 {
    (noc & !0xFF00000000000000) | ((noc & 0xFF00000000000000) >> 16)
}

#[no_mangle]
pub extern "C" fn phys_to_noc(phys: u64) -> u64 {
    (phys & !0x0000_FF00_0000_0000) | ((phys & 0x0000_FF00_0000_0000) << 16)
}

pub fn get_pte_addr(mut virt: usize, level: usize) -> usize {
    #[allow(clippy::erasing_op)]
    #[rustfmt::skip]
    const REC_MASK: usize = ((PTE_REC_IDX << (cfg::PAGE_BITS + LEVEL_BITS * 3))
                           | (PTE_REC_IDX << (cfg::PAGE_BITS + LEVEL_BITS * 2))
                           | (PTE_REC_IDX << (cfg::PAGE_BITS + LEVEL_BITS * 1))
                           | (PTE_REC_IDX << (cfg::PAGE_BITS + LEVEL_BITS * 0)));

    // at first, just shift it accordingly.
    virt >>= cfg::PAGE_BITS + level * LEVEL_BITS;
    virt <<= PTE_BITS;

    // now put in one PTE_REC_IDX's for each loop that we need to take
    let shift = level + 1;
    let rem_mask = (1 << (cfg::PAGE_BITS + LEVEL_BITS * (LEVEL_CNT - shift))) - 1;
    virt |= REC_MASK & !rem_mask;

    // finally, make sure that we stay within the bounds for virtual addresses
    // this is because of recMask, that might actually have too many of those.
    virt &= (1 << (LEVEL_CNT * LEVEL_BITS + cfg::PAGE_BITS)) - 1;
    virt
}
