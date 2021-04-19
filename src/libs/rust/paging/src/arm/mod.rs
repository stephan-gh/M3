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
use base::kif::{pemux, PageFlags};
use bitflags::bitflags;

pub type MMUPTE = u64;
pub type Phys = u64;

pub const PTE_BITS: usize = 3;

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
    pub fn is_leaf(self, level: usize) -> bool {
        level == 0 || (self.bits() & Self::TYPE.bits()) != Self::TBL.bits()
    }

    pub fn perms_missing(self, perms: Self) -> bool {
        !self.contains(Self::P)
            || (self.contains(Self::NW) && !perms.contains(Self::NW))
            || (self.contains(Self::NX) && !perms.contains(Self::NX))
    }
}

pub fn build_pte(phys: Phys, perm: MMUFlags, level: usize, leaf: bool) -> MMUPTE {
    let pte = phys | perm.bits();
    if leaf {
        if level > 0 {
            pte | (MMUFlags::BLK | MMUFlags::NG).bits()
        }
        else {
            pte | (MMUFlags::PAGE | MMUFlags::NG).bits()
        }
    }
    else {
        pte | (MMUFlags::TBL | MMUFlags::A | MMUFlags::NG).bits()
    }
}

pub fn pte_to_phys(pte: MMUPTE) -> Phys {
    pte & !MMUFlags::FLAGS.bits()
}

pub fn needs_invalidate(_new_flags: MMUFlags, old_flags: MMUFlags) -> bool {
    // invalidate the TLB entry on every change
    old_flags.bits() != 0
}

pub fn to_page_flags(_level: usize, pte: MMUFlags) -> PageFlags {
    let mut res = PageFlags::empty();
    if pte.contains(MMUFlags::P) {
        res |= PageFlags::R;
    }
    else {
        return res;
    }
    if !pte.contains(MMUFlags::NW) {
        res |= PageFlags::W;
    }
    if pte.contains(MMUFlags::U) {
        res |= PageFlags::U;
    }
    if !pte.contains(MMUFlags::NX) {
        res |= PageFlags::X;
    }
    if (pte & MMUFlags::TYPE).bits() == MMUFlags::BLK.bits() {
        res |= PageFlags::L;
    }
    res
}

pub fn to_mmu_perms(flags: PageFlags) -> MMUFlags {
    let mut res = MMUFlags::empty();
    if flags.intersects(PageFlags::RWX) {
        res |= MMUFlags::P | MMUFlags::A;
    }
    if !flags.contains(PageFlags::W) {
        res |= MMUFlags::NW;
    }
    if flags.contains(PageFlags::U) {
        res |= MMUFlags::U;
    }
    if !flags.contains(PageFlags::X) {
        res |= MMUFlags::NX;
    }
    res
}

pub fn enable_paging() {
    unsafe {
        llvm_asm!("
            mrc     p15, 0, r0, c2, c0, 2;   // TTBCR
            orr     r0, r0, #0x80000000;     // EAE = 1 (40-bit translation system with long table format)
            orr     r0, r0, #0x00000500;     // ORGN0 = IRGN0 = 1 (write-back write-allocate cacheable)
            mcr     p15, 0, r0, c2, c0, 2;
            mrc     p15, 0, r0, c10, c2, 0;  // MAIR0
            orr     r0, r0, #0xFF;           // normal memory, write-back, rw-alloc, cacheable
            mcr     p15, 0, r0, c10, c2, 0;
            mrc     p15, 0, r0, c1, c0, 0;   // SCTLR
            orr     r0, r0, #0x00000001;     // enable MMU
            mcr     p15, 0, r0, c1, c0, 0;
            "
            : : : "r0" : "volatile"
        );
    }
}

pub fn disable_paging() {
    // not necessary
}

pub fn invalidate_page(id: crate::VPEId, virt: usize) {
    unsafe {
        llvm_asm!(
            "mcr p15, 0, $0, c8, c7, 1"
            : : "r"(virt | (id as usize & 0xFF))
            : : "volatile"
        );
    }
}

pub fn invalidate_tlb() {
    // note that r0 is ignored
    unsafe {
        llvm_asm!(
            "mcr p15, 0, r0, c8, c7, 0"
            : : : : "volatile"
        );
    }
}

pub fn set_root_pt(id: crate::VPEId, root: Phys) {
    // the ASID is 8 bit; make sure that we stay in that space
    assert!(
        id == pemux::VPE_ID
            || id == pemux::IDLE_ID
            || (id != pemux::VPE_ID & 0xFF && id != pemux::IDLE_ID & 0xFF)
    );
    // cacheable table walk, non-shareable, outer write-back write-allocate cacheable
    let ttbr0_low: u32 = (root | 0b00_1001) as u32;
    let ttbr0_high: u32 = ((id as u32 & 0xFF) << 16) | (root >> 32) as u32;
    unsafe {
        llvm_asm!("
             mcrr p15, 0, $0, $1, c2;
             // synchronize changes to control register
             .arch armv7;
             isb;
             "
            : : "r"(ttbr0_low), "r"(ttbr0_high)
            : : "volatile"
        );
    }
}
