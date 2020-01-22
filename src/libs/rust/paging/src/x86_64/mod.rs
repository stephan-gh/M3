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
use base::errors::{Code, Error};
use base::goff;
use base::kif::{PageFlags, PTE};
use base::math;
use base::util;
use core::fmt;

use crate::{AllocFrameFunc, XlatePtFunc};

pub type MMUPTE = usize;

pub const PTE_BITS: usize = 3;
pub const PTE_SIZE: usize = 1 << PTE_BITS;
pub const PTE_REC_IDX: usize = 0x10;

pub const LEVEL_CNT: usize = 4;
pub const LEVEL_BITS: usize = cfg::PAGE_BITS - PTE_BITS;
pub const LEVEL_MASK: usize = (1 << LEVEL_BITS) - 1;
pub const LPAGE_BITS: usize = cfg::PAGE_BITS + LEVEL_BITS;

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

#[no_mangle]
pub extern "C" fn to_map_flags(pte: MMUFlags) -> PageFlags {
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

fn to_mmu_flags(flags: PageFlags) -> MMUFlags {
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

fn to_pte(pte: MMUPTE) -> PTE {
    let res = phys_to_noc((pte & !MMUFlags::FLAGS.bits()) as u64);
    let flags = MMUFlags::from_bits_truncate(pte);
    res | to_map_flags(flags).bits()
}

fn invalidate_page(virt: usize) {
    unsafe { asm!("invlpg ($0)" : : "r"(virt)) }
}

#[no_mangle]
pub extern "C" fn get_addr_space() -> PTE {
    let addr: MMUPTE;
    unsafe { asm!("mov %cr3, $0" : "=r"(addr)) };
    phys_to_noc(addr as u64)
}

fn set_addr_space(addr: MMUPTE) {
    unsafe { asm!("mov $0, %cr3" : : "r"(addr)) };
}

#[no_mangle]
pub extern "C" fn noc_to_phys(noc: u64) -> u64 {
    (noc & !0xFF00000000000000) | ((noc & 0xFF00000000000000) >> 16)
}

#[no_mangle]
pub extern "C" fn phys_to_noc(phys: u64) -> u64 {
    (phys & !0x0000_FF00_0000_0000) | ((phys & 0x0000_FF00_0000_0000) << 16)
}

fn get_pte_addr(mut virt: usize, level: usize) -> usize {
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

fn get_pte_at(virt: usize, level: usize) -> MMUPTE {
    let virt = get_pte_addr(virt, level);
    // safety: we can access that address because of our recursive entry
    unsafe { *(virt as *const MMUPTE) }
}

#[no_mangle]
pub extern "C" fn translate(virt: usize, perm: PTE) -> PTE {
    // translate to physical
    if (virt & 0xFFFF_FFFF_F000) == 0x0804_0201_0000 {
        // special case for root pt
        let pte = get_addr_space();
        pte | (PageFlags::RW).bits()
    }
    else if (virt & 0xFFF0_0000_0000) == 0x0800_0000_0000 {
        // in the MMUPTE area, we can assume that all upper level PTEs are present
        to_pte(get_pte_at(virt, 0))
    }
    else {
        // ignore the executable bit here
        let mmu_perm = to_mmu_flags(PageFlags::from_bits_truncate(perm) | PageFlags::X);
        // otherwise, walk through all levels
        for lvl in (0..LEVEL_CNT).rev() {
            let pte = get_pte_at(virt, lvl);
            if lvl == 0
                || (!(pte & MMUFlags::RW.bits()) & mmu_perm.bits()) != 0
                || (pte & MMUFlags::L.bits()) != 0
            {
                return to_pte(pte);
            }
        }
        unreachable!();
    }
}

#[no_mangle]
pub extern "C" fn map_pages(
    id: u64,
    virt: usize,
    noc: goff,
    pages: usize,
    perm: PTE,
    alloc_frame: AllocFrameFunc,
    xlate_pt: XlatePtFunc,
    root: goff,
) {
    let aspace = AddrSpace::new(id, root, xlate_pt, alloc_frame);
    let perm = PageFlags::from_bits_truncate(perm);
    aspace.map_pages(virt, noc, pages, perm).unwrap();
}

pub struct AddrSpace {
    id: u64,
    root: MMUPTE,
    xlate_pt: XlatePtFunc,
    alloc_frame: AllocFrameFunc,
}

impl AddrSpace {
    pub fn new(id: u64, root: goff, xlate_pt: XlatePtFunc, alloc_frame: AllocFrameFunc) -> Self {
        AddrSpace {
            id,
            root: noc_to_phys(root) as MMUPTE,
            xlate_pt,
            alloc_frame,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn switch_to(&self) {
        set_addr_space(self.root);
    }

    pub fn init(&self) {
        let pt_virt = (self.xlate_pt)(self.id, self.root);
        Self::clear_pt(pt_virt);

        // insert recursive entry
        let rec_idx_pte = pt_virt + PTE_REC_IDX * util::size_of::<MMUPTE>();
        // safety: we can access that address because `xlate_pt` returns as a mapped page for the
        // whole page table
        unsafe { *(rec_idx_pte as *mut MMUPTE) = self.root | MMUFlags::RWX.bits() };
    }

    pub fn map_pages(
        &self,
        mut virt: usize,
        noc: goff,
        mut pages: usize,
        perm: PageFlags,
    ) -> Result<(), Error> {
        log!(
            crate::LOG_MAP,
            "VPE{}: mapping 0x{:0>16x}..0x{:0>16x} to 0x{:0>16x}..0x{:0>16x} with {:?}",
            self.id,
            virt,
            virt + pages * cfg::PAGE_SIZE - 1,
            noc,
            noc + (pages * cfg::PAGE_SIZE) as goff - 1,
            perm
        );

        let lvl = LEVEL_CNT - 1;
        let mut phys = noc_to_phys(noc) as MMUPTE;
        let perm = to_mmu_flags(perm);
        self.map_pages_rec(&mut virt, &mut phys, &mut pages, perm, self.root, lvl)
    }

    fn map_pages_rec(
        &self,
        virt: &mut usize,
        phys: &mut MMUPTE,
        pages: &mut usize,
        perm: MMUFlags,
        pte: MMUPTE,
        level: usize,
    ) -> Result<(), Error> {
        // determine virtual address for page table
        let pt_virt = (self.xlate_pt)(self.id, pte & !MMUFlags::FLAGS.bits());

        // start at the corresponding index
        let idx = (*virt >> (cfg::PAGE_BITS + level * LEVEL_BITS)) & LEVEL_MASK;
        let mut pte_addr = pt_virt + idx * util::size_of::<MMUPTE>();

        while *pages > 0 {
            // reached end of page table?
            if pte_addr >= pt_virt + cfg::PAGE_SIZE {
                break;
            }

            // safety: as above
            let mut pte = unsafe { *(pte_addr as *const MMUPTE) };

            // can we use a large page?
            if level == 0
                || (level == 1
                    && math::is_aligned(*virt, cfg::LPAGE_SIZE)
                    && math::is_aligned(*phys, cfg::LPAGE_SIZE)
                    && *pages * cfg::PAGE_SIZE >= cfg::LPAGE_SIZE)
            {
                let (psize, flags) = if level == 1 {
                    (cfg::LPAGE_SIZE, MMUFlags::L.bits())
                }
                else {
                    (cfg::PAGE_SIZE, 0)
                };

                let new_pte = *phys | perm.bits() | flags;

                // determine if we need to perform an TLB invalidate
                let rwx = MMUFlags::RWX.bits() as MMUPTE;
                let downgrade = (pte & rwx) != 0 && ((pte & rwx) & (!new_pte & rwx)) != 0;
                if downgrade {
                    invalidate_page(*virt);
                }

                // safety: as above
                unsafe { *(pte_addr as *mut MMUPTE) = new_pte };

                log!(
                    crate::LOG_MAP_DETAIL,
                    "VPE{}: lvl {} PTE for 0x{:0>16x}: 0x{:0>16x} (downgrade={})",
                    self.id,
                    level,
                    virt,
                    new_pte,
                    downgrade
                );

                *pages -= psize / cfg::PAGE_SIZE;
                *virt += psize;
                *phys += psize;
            }
            else {
                // unmapping non-existing PTs is a noop
                if !(pte == 0 && perm.is_empty()) {
                    if pte == 0 {
                        pte = self.create_pt(*virt, pte_addr, level)?;
                    }

                    self.map_pages_rec(virt, phys, pages, perm, pte, level - 1)?;
                }
            }

            pte_addr += util::size_of::<MMUPTE>();
        }

        Ok(())
    }

    fn create_pt(&self, virt: usize, pte_addr: usize, level: usize) -> Result<MMUPTE, Error> {
        let frame = (self.alloc_frame)(self.id);
        if frame == 0 {
            return Err(Error::new(Code::NoSpace));
        }
        Self::clear_pt((self.xlate_pt)(self.id, frame));

        // insert MMUPTE
        let pte = frame | MMUFlags::RWX.bits() | MMUFlags::U.bits();
        // safety: as above
        unsafe { *(pte_addr as *mut MMUPTE) = pte };

        let pt_size = (1 << (LEVEL_BITS * level)) * cfg::PAGE_SIZE;
        let virt_base = virt as usize & !(pt_size - 1);
        log!(
            crate::LOG_MAP_DETAIL,
            "VPE{}: lvl {} PTE for 0x{:0>16x}: 0x{:0>16x}",
            self.id,
            level,
            virt_base,
            pte
        );

        Ok(pte)
    }

    fn clear_pt(mut pt_virt: usize) {
        for _ in 0..1 << LEVEL_BITS {
            // safety: as above
            unsafe { *(pt_virt as *mut MMUPTE) = 0 };
            pt_virt += util::size_of::<MMUPTE>();
        }
    }

    fn print_as_rec(&self, f: &mut fmt::Formatter<'_>, pt: MMUPTE, mut virt: usize, level: usize) {
        let mut ptes = (self.xlate_pt)(self.id, pt);
        for _ in 0..1 << LEVEL_BITS {
            // safety: as above
            let pte = unsafe { *(ptes as *const MMUPTE) };
            if pte != 0 {
                let w = (LEVEL_CNT - level - 1) * 2;
                writeln!(f, "{:w$}0x{:0>16x}: 0x{:0>16x}", "", virt, pte, w = w).ok();
                if level > 0 && (pte & MMUFlags::L.bits()) == 0 {
                    let pt = pte & !MMUFlags::FLAGS.bits();
                    self.print_as_rec(f, pt, virt, level - 1);
                }
            }

            virt += 1 << (level as usize * LEVEL_BITS + cfg::PAGE_BITS);
            ptes += util::size_of::<MMUPTE>();

            // don't enter the MMUPTE area
            if virt >= 0x0800_0000_0000 {
                break;
            }
        }
    }
}

// TODO implement Drop to free the page tables

impl fmt::Debug for AddrSpace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        writeln!(f, "Address space @ 0x{:0>16x}:", self.root)?;
        self.print_as_rec(f, self.root, 0, LEVEL_CNT - 1);
        Ok(())
    }
}
