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

use base::cfg;
use base::kif::{tilemux, PageFlags};
use base::mem::{PhysAddr, PhysAddrRaw, VirtAddr};

use bitflags::bitflags;

use core::arch::asm;

use crate::ArchMMUFlags;

pub type MMUPTE = u64;

pub const PTE_BITS: usize = 3;

pub const LEVEL_CNT: usize = 3;
pub const LEVEL_BITS: usize = cfg::PAGE_BITS - PTE_BITS;
pub const LEVEL_MASK: usize = (1 << LEVEL_BITS) - 1;

bitflags! {
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub struct ARMMMUFlags : MMUPTE {
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

        const RW    = Self::A.bits() | Self::P.bits() | Self::NX.bits();
        const RWX   = Self::A.bits() | Self::P.bits();

        const FLAGS = cfg::PAGE_MASK as u64 | Self::NX.bits();
    }
}

impl ArchMMUFlags for ARMMMUFlags {
    fn has_empty_perm(&self) -> bool {
        !self.contains(Self::P)
    }

    fn is_leaf(&self, level: usize) -> bool {
        level == 0 || (self.bits() & Self::TYPE.bits()) != Self::TBL.bits()
    }

    fn access_allowed(&self, flags: Self) -> bool {
        self.contains(Self::P)
            && !(self.contains(Self::NW) && !flags.contains(Self::NW))
            && !(self.contains(Self::NX) && !flags.contains(Self::NX))
    }
}

pub struct ARMPaging {}

impl crate::ArchPaging for ARMPaging {
    type MMUFlags = ARMMMUFlags;

    fn build_pte(phys: PhysAddr, perm: Self::MMUFlags, level: usize, leaf: bool) -> MMUPTE {
        let pte = phys.as_raw() as MMUPTE | perm.bits();
        if leaf {
            if perm.has_empty_perm() {
                0
            }
            else if level > 0 {
                pte | (Self::MMUFlags::BLK | Self::MMUFlags::NG).bits()
            }
            else {
                pte | (Self::MMUFlags::PAGE | Self::MMUFlags::NG).bits()
            }
        }
        else {
            pte | (Self::MMUFlags::TBL | Self::MMUFlags::A | Self::MMUFlags::NG).bits()
        }
    }

    fn pte_to_phys(pte: MMUPTE) -> PhysAddr {
        PhysAddr::new_raw((pte & !Self::MMUFlags::FLAGS.bits()) as PhysAddrRaw)
    }

    fn needs_invalidate(_new_flags: Self::MMUFlags, old_flags: Self::MMUFlags) -> bool {
        // invalidate the TLB entry on every change
        old_flags.bits() != 0
    }

    fn to_page_flags(_level: usize, pte: Self::MMUFlags) -> PageFlags {
        let mut res = PageFlags::empty();
        if pte.contains(Self::MMUFlags::P) {
            res |= PageFlags::R;
        }
        else {
            return res;
        }
        if !pte.contains(Self::MMUFlags::NW) {
            res |= PageFlags::W;
        }
        if pte.contains(Self::MMUFlags::U) {
            res |= PageFlags::U;
        }
        if !pte.contains(Self::MMUFlags::NX) {
            res |= PageFlags::X;
        }
        if (pte & Self::MMUFlags::TYPE).bits() == Self::MMUFlags::BLK.bits() {
            res |= PageFlags::L;
        }
        res
    }

    fn to_mmu_perms(flags: PageFlags) -> Self::MMUFlags {
        let mut res = Self::MMUFlags::empty();
        if flags.intersects(PageFlags::RWX) {
            res |= Self::MMUFlags::P | Self::MMUFlags::A;
        }
        if !flags.contains(PageFlags::W) {
            res |= Self::MMUFlags::NW;
        }
        if flags.contains(PageFlags::U) {
            res |= Self::MMUFlags::U;
        }
        if !flags.contains(PageFlags::X) {
            res |= Self::MMUFlags::NX;
        }
        res
    }

    fn enable() {
        unsafe {
            asm!(
                "mrc     p15, 0, r0, c2, c0, 2",   // TTBCR
                "orr     r0, r0, #0x80000000",     // EAE = 1 (40-bit translation system with long table format)
                "orr     r0, r0, #0x00000500",     // ORGN0 = IRGN0 = 1 (write-back write-allocate cacheable)
                "mcr     p15, 0, r0, c2, c0, 2",
                "mrc     p15, 0, r0, c10, c2, 0",  // MAIR0
                "orr     r0, r0, #0xFF",           // normal memory, write-back, rw-alloc, cacheable
                "mcr     p15, 0, r0, c10, c2, 0",
                "mrc     p15, 0, r0, c1, c0, 0",   // SCTLR
                "orr     r0, r0, #0x00000001",     // enable MMU
                "mcr     p15, 0, r0, c1, c0, 0",
                lateout("r0") _,
            );
        }
    }

    fn disable() {
        // not necessary
    }

    fn invalidate_page(id: crate::ActId, virt: VirtAddr) {
        let val = virt.as_local() | (id as usize & 0xFF);
        unsafe {
            asm!(
                "mcr p15, 0, {0}, c8, c7, 1",
                in(reg) val,
                options(nostack),
            );
        }
    }

    fn invalidate_tlb() {
        // note that r0 is ignored
        unsafe {
            asm!("mcr p15, 0, r0, c8, c7, 0", options(nostack));
        }
    }

    fn set_root_pt(id: crate::ActId, root: PhysAddr) {
        // the ASID is 8 bit; make sure that we stay in that space
        assert!(
            id == tilemux::ACT_ID
                || id == tilemux::IDLE_ID
                || (id != tilemux::ACT_ID & 0xFF && id != tilemux::IDLE_ID & 0xFF)
        );
        // cacheable table walk, non-shareable, outer write-back write-allocate cacheable
        let ttbr0_low: u32 = (root.as_raw() | 0b00_1001) as u32;
        let ttbr0_high: u32 = (id as u32 & 0xFF) << 16;
        unsafe {
            asm!(
                 "mcrr p15, 0, {0}, {1}, c2",
                 // synchronize changes to control register
                 ".arch armv7",
                 "isb",
                 in(reg) ttbr0_low,
                 in(reg) ttbr0_high,
                 options(nostack),
            );
        }
    }
}
