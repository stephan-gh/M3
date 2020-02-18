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

#![feature(asm)]
#![no_std]

#[macro_use]
extern crate base;
#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate cfg_if;

cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        #[path = "x86_64/mod.rs"]
        mod arch;
    }
    else if #[cfg(target_arch = "arm")] {
        #[path = "arm/mod.rs"]
        mod arch;
    }
    else if #[cfg(target_arch = "riscv64")] {
        #[path = "riscv/mod.rs"]
        mod arch;
    }
}

use base::cfg;
use base::errors::{Code, Error};
use base::goff;
use base::kif::{PageFlags, PTE};
use base::math;
use base::util;
use core::fmt;

use arch::{LEVEL_BITS, LEVEL_CNT, LEVEL_MASK, PTE_REC_IDX};

pub use arch::{
    build_pte, enable_paging, noc_to_phys, phys_to_noc, pte_to_phys, to_page_flags, MMUFlags,
    MMUPTE,
};

/// Logs mapping operations
pub const LOG_MAP: bool = false;
/// Logs detailed mapping operations
pub const LOG_MAP_DETAIL: bool = false;

pub type AllocFrameFunc = extern "C" fn(vpe: u64) -> MMUPTE;
pub type XlatePtFunc = extern "C" fn(vpe: u64, phys: MMUPTE) -> usize;

#[no_mangle]
pub extern "C" fn init_aspace(
    id: u64,
    alloc_frame: AllocFrameFunc,
    xlate_pt: XlatePtFunc,
    root: goff,
) {
    // TODO don't flush TLB on destruction
    let aspace = AddrSpace::new(id, root, xlate_pt, alloc_frame);
    aspace.init();
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

#[no_mangle]
pub extern "C" fn get_addr_space() -> PTE {
    arch::phys_to_noc(arch::get_root_pt() as u64)
}

#[no_mangle]
pub extern "C" fn set_addr_space(root: PTE, alloc_frame: AllocFrameFunc, xlate_pt: XlatePtFunc) {
    let aspace = AddrSpace::new(0, root, xlate_pt, alloc_frame);
    aspace.switch_to();
}

fn to_pte(level: usize, pte: MMUPTE) -> PTE {
    let res = phys_to_noc(pte_to_phys(pte) as u64);
    let flags = MMUFlags::from_bits_truncate(pte);
    res | to_page_flags(level, flags).bits()
}

#[no_mangle]
pub extern "C" fn translate(
    id: u64,
    root: PTE,
    alloc_frame: AllocFrameFunc,
    xlate_pt: XlatePtFunc,
    virt: usize,
    perm: PTE,
) -> PTE {
    let aspace = AddrSpace::new(id, root, xlate_pt, alloc_frame);
    aspace.translate(virt, perm)
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
            root: build_pte(
                arch::noc_to_phys(root) as MMUPTE,
                MMUFlags::empty(),
                LEVEL_CNT,
                false,
            ),
            xlate_pt,
            alloc_frame,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn switch_to(&self) {
        arch::set_root_pt(self.id, pte_to_phys(self.root));
    }

    pub fn init(&self) {
        let root_phys = pte_to_phys(self.root);
        let pt_virt = (self.xlate_pt)(self.id, root_phys);
        Self::clear_pt(pt_virt);

        // insert recursive entry
        let rec_idx_pte = pt_virt + PTE_REC_IDX * util::size_of::<MMUPTE>();
        let new_pte = build_pte(root_phys, MMUFlags::empty(), LEVEL_CNT - 1, false);
        // safety: we can access that address because `xlate_pt` returns as a mapped page for the
        // whole page table
        unsafe { *(rec_idx_pte as *mut MMUPTE) = new_pte };
    }

    pub fn translate(&self, virt: usize, perm: PTE) -> PTE {
        // otherwise, walk through all levels
        let perm = arch::to_mmu_perms(PageFlags::from_bits_truncate(perm));
        let mut pte = self.root;
        for lvl in (0..LEVEL_CNT).rev() {
            let pt_virt = (self.xlate_pt)(self.id, pte_to_phys(pte));
            let idx = (virt >> (cfg::PAGE_BITS + lvl * LEVEL_BITS)) & LEVEL_MASK;
            let pte_addr = pt_virt + idx * util::size_of::<MMUPTE>();

            // safety: as above
            pte = unsafe { *(pte_addr as *const MMUPTE) };

            let pte_flags = MMUFlags::from_bits_truncate(pte);
            if pte_flags.is_leaf(lvl) || pte_flags.perms_missing(perm) {
                return to_pte(lvl, pte);
            }
        }
        unreachable!();
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
        let mut phys = arch::noc_to_phys(noc) as MMUPTE;
        let perm = arch::to_mmu_perms(perm);
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
        let pt_virt = (self.xlate_pt)(self.id, pte_to_phys(pte));

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
                    && math::is_aligned(*phys, cfg::LPAGE_SIZE as MMUPTE)
                    && *pages * cfg::PAGE_SIZE >= cfg::LPAGE_SIZE)
            {
                let psize = if level == 1 {
                    cfg::LPAGE_SIZE
                }
                else {
                    cfg::PAGE_SIZE
                };

                let new_pte = build_pte(*phys, perm, level, true);

                // determine if we need to perform an TLB invalidate
                let old_flags = MMUFlags::from_bits_truncate(pte);
                let new_flags = MMUFlags::from_bits_truncate(new_pte);
                let invalidate = arch::needs_invalidate(new_flags, old_flags);
                if invalidate {
                    arch::invalidate_page(self.id, *virt);
                }

                // safety: as above
                unsafe { *(pte_addr as *mut MMUPTE) = new_pte };

                log!(
                    crate::LOG_MAP_DETAIL,
                    "VPE{}: lvl {} PTE for 0x{:0>16x}: 0x{:0>16x} (invalidate={})",
                    self.id,
                    level,
                    virt,
                    new_pte,
                    invalidate
                );

                *pages -= psize / cfg::PAGE_SIZE;
                *virt += psize;
                *phys += psize as MMUPTE;
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

        // insert PTE
        let pte = build_pte(frame, MMUFlags::empty(), level, false);
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

    fn print_as_rec(
        &self,
        f: &mut fmt::Formatter<'_>,
        pt: MMUPTE,
        mut virt: usize,
        level: usize,
    ) -> Result<(), fmt::Error> {
        let pte_begin = PTE_REC_IDX << (cfg::PAGE_BITS + (LEVEL_CNT - 1) * LEVEL_BITS);
        let pte_end = pte_begin + (1 << (cfg::PAGE_BITS + (LEVEL_CNT - 1) * LEVEL_BITS));

        let mut ptes = (self.xlate_pt)(self.id, pt);
        for _ in 0..1 << LEVEL_BITS {
            // don't enter the MMUPTE area
            if virt < pte_begin || virt >= pte_end {
                // safety: as above
                let pte = unsafe { *(ptes as *const MMUPTE) };
                if pte != 0 {
                    let w = (LEVEL_CNT - level - 1) * 2;
                    writeln!(f, "{:w$}0x{:0>16x}: 0x{:0>16x}", "", virt, pte, w = w)?;
                    if !MMUFlags::from_bits_truncate(pte).is_leaf(level) {
                        let pt = pte_to_phys(pte);
                        self.print_as_rec(f, pt, virt, level - 1)?;
                    }
                }
            }

            virt += 1 << (level as usize * LEVEL_BITS + cfg::PAGE_BITS);
            ptes += util::size_of::<MMUPTE>();
        }
        Ok(())
    }
}

impl Drop for AddrSpace {
    fn drop(&mut self) {
        // invalidate entire TLB to allow us to reuse the VPE id
        arch::invalidate_tlb();

        // TODO free the page tables
    }
}

impl fmt::Debug for AddrSpace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        writeln!(f, "Address space @ 0x{:0>16x}:", pte_to_phys(self.root))?;
        self.print_as_rec(f, self.root, 0, LEVEL_CNT - 1)
    }
}