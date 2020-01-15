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
use base::dtu;
use base::errors::{Code, Error};
use base::goff;
use base::math;
use base::util;
use core::fmt;

use crate::{AllocFrameFunc, XlatePtFunc};

pub type PTE = usize;

bitflags! {
    pub struct MMUFlags : PTE {
        const PRESENT       = 0b0000_0001;
        const WRITE         = 0b0000_0010;
        const USER          = 0b0000_0100;
        const UNCACHED      = 0b0001_0000;
        const LARGE         = 0b1000_0000;
        const NOEXEC        = 0x8000_0000_0000_0000;
    }
}

fn to_mmu_pte(pte: dtu::PTE) -> PTE {
    let res = pte & !cfg::PAGE_MASK as u64;
    let mut res = noc_to_phys(res) as PTE;

    if (pte & dtu::PTEFlags::RWX.bits()) != 0 {
        res |= MMUFlags::PRESENT.bits();
    }
    if (pte & dtu::PTEFlags::W.bits()) != 0 {
        res |= MMUFlags::WRITE.bits();
    }
    if (pte & dtu::PTEFlags::I.bits()) != 0 {
        res |= MMUFlags::USER.bits();
    }
    if (pte & dtu::PTEFlags::UNCACHED.bits()) != 0 {
        res |= MMUFlags::UNCACHED.bits();
    }
    if (pte & dtu::PTEFlags::LARGE.bits()) != 0 {
        res |= MMUFlags::LARGE.bits();
    }
    if (pte & dtu::PTEFlags::X.bits()) == 0 {
        res |= MMUFlags::NOEXEC.bits();
    }
    res
}

#[no_mangle]
pub extern "C" fn to_dtu_pte(pte: PTE) -> dtu::PTE {
    if pte == 0 {
        return 0;
    }

    let res = (pte & !cfg::PAGE_MASK as PTE) as dtu::PTE;
    let mut res = phys_to_noc(res);

    if (pte & MMUFlags::PRESENT.bits()) != 0 {
        res |= dtu::PTEFlags::R.bits();
    }
    if (pte & MMUFlags::WRITE.bits()) != 0 {
        res |= dtu::PTEFlags::W.bits();
    }
    if (pte & MMUFlags::USER.bits()) != 0 {
        res |= dtu::PTEFlags::I.bits();
    }
    if (pte & MMUFlags::LARGE.bits()) != 0 {
        res |= dtu::PTEFlags::LARGE.bits();
    }
    if (pte & MMUFlags::NOEXEC.bits()) == 0 {
        res |= dtu::PTEFlags::X.bits();
    }
    res
}

fn invalidate_page(virt: usize) {
    unsafe { asm!("invlpg ($0)" : : "r"(virt)) }
}

#[no_mangle]
pub extern "C" fn get_addr_space() -> PTE {
    let addr: PTE;
    unsafe { asm!("mov %cr3, $0" : "=r"(addr)) };
    addr
}

#[no_mangle]
pub extern "C" fn set_addr_space(addr: PTE) {
    unsafe { asm!("mov $0, %cr3" : : "r"(noc_to_phys(addr as u64))) };
}

#[no_mangle]
pub extern "C" fn noc_to_phys(noc: u64) -> u64 {
    (noc & !0xFF00000000000000) | ((noc & 0xFF00000000000000) >> 16)
}

#[no_mangle]
pub extern "C" fn phys_to_noc(phys: u64) -> u64 {
    (phys & !0x0000_FF00_0000_0000) | ((phys & 0x0000_FF00_0000_0000) << 16)
}

fn get_pte_addr(mut virt: usize, level: u32) -> usize {
    #[allow(clippy::erasing_op)]
    #[rustfmt::skip]
    const REC_MASK: usize = ((cfg::PTE_REC_IDX << (cfg::PAGE_BITS + cfg::LEVEL_BITS * 3))
                           | (cfg::PTE_REC_IDX << (cfg::PAGE_BITS + cfg::LEVEL_BITS * 2))
                           | (cfg::PTE_REC_IDX << (cfg::PAGE_BITS + cfg::LEVEL_BITS * 1))
                           | (cfg::PTE_REC_IDX << (cfg::PAGE_BITS + cfg::LEVEL_BITS * 0)));

    // at first, just shift it accordingly.
    virt >>= cfg::PAGE_BITS + level as usize * cfg::LEVEL_BITS;
    virt <<= cfg::PTE_BITS;

    // now put in one PTE_REC_IDX's for each loop that we need to take
    let shift = (level + 1) as usize;
    let rem_mask = (1 << (cfg::PAGE_BITS + cfg::LEVEL_BITS * (cfg::LEVEL_CNT - shift))) - 1;
    virt |= REC_MASK & !rem_mask;

    // finally, make sure that we stay within the bounds for virtual addresses
    // this is because of recMask, that might actually have too many of those.
    virt &= (1 << (cfg::LEVEL_CNT * cfg::LEVEL_BITS + cfg::PAGE_BITS)) - 1;
    virt
}

#[no_mangle]
pub extern "C" fn get_pte_at(virt: usize, level: u32) -> PTE {
    let virt = get_pte_addr(virt, level);
    unsafe { *(virt as *const PTE) }
}

#[no_mangle]
pub extern "C" fn get_pte(virt: usize, perm: u64) -> dtu::PTE {
    // translate to physical
    if (virt & 0xFFFF_FFFF_F000) == 0x0804_0201_0000 {
        // special case for root pt
        let pte = get_addr_space();
        to_dtu_pte(pte | (MMUFlags::PRESENT | MMUFlags::WRITE).bits())
    }
    else if (virt & 0xFFF0_0000_0000) == 0x0800_0000_0000 {
        // in the PTE area, we can assume that all upper level PTEs are present
        to_dtu_pte(get_pte_at(virt, 0))
    }
    else {
        // otherwise, walk through all levels
        for lvl in (0..cfg::LEVEL_CNT as u32).rev() {
            let pte = to_dtu_pte(get_pte_at(virt, lvl));
            if lvl == 0
                || (!(pte & dtu::PTEFlags::IRWX.bits()) & perm) != 0
                || (pte & dtu::PTEFlags::LARGE.bits()) != 0
            {
                return pte;
            }
        }
        unreachable!();
    }
}

#[no_mangle]
pub extern "C" fn map_pages(
    vpe: u64,
    virt: usize,
    phys: goff,
    pages: usize,
    perm: u64,
    alloc_frame: AllocFrameFunc,
    xlate_pt: XlatePtFunc,
    root: goff,
) {
    let aspace = AddrSpace::new(vpe, root, xlate_pt, alloc_frame);
    aspace
        .map_pages(virt, phys, pages, dtu::PTEFlags::from_bits_truncate(perm))
        .unwrap();
}

pub struct AddrSpace {
    pub vpe: u64,
    pub root: goff,
    xlate_pt: XlatePtFunc,
    alloc_frame: AllocFrameFunc,
}

impl AddrSpace {
    pub fn new(vpe: u64, root: goff, xlate_pt: XlatePtFunc, alloc_frame: AllocFrameFunc) -> Self {
        AddrSpace {
            vpe,
            root,
            xlate_pt,
            alloc_frame,
        }
    }

    pub fn init(&self) {
        let pt_virt = (self.xlate_pt)(self.vpe, self.root);
        Self::clear_pt(pt_virt);

        // insert recursive entry
        let rec_idx_pte = pt_virt + cfg::PTE_REC_IDX * util::size_of::<PTE>();
        unsafe { *(rec_idx_pte as *mut PTE) = to_mmu_pte(self.root | dtu::PTEFlags::RWX.bits()) };
    }

    pub fn map_pages(
        &self,
        mut virt: usize,
        mut phys: goff,
        mut pages: usize,
        perm: dtu::PTEFlags,
    ) -> Result<(), Error> {
        log!(
            crate::LOG_MAP,
            "VPE{}: mapping 0x{:0>16x}..0x{:0>16x} to 0x{:0>16x}..0x{:0>16x} with {:?}",
            self.vpe,
            virt,
            virt + pages * cfg::PAGE_SIZE,
            phys,
            phys + (pages * cfg::PAGE_SIZE) as goff,
            perm
        );

        let root = to_dtu_pte(self.root as PTE);
        let lvl = cfg::LEVEL_CNT - 1;
        self.map_pages_rec(&mut virt, &mut phys, &mut pages, perm, root, lvl)
    }

    fn map_pages_rec(
        &self,
        virt: &mut usize,
        phys: &mut goff,
        pages: &mut usize,
        perm: dtu::PTEFlags,
        pte: dtu::PTE,
        level: usize,
    ) -> Result<(), Error> {
        // determine virtual address for page table
        let pt_virt = (self.xlate_pt)(self.vpe, pte as goff) & !cfg::PAGE_MASK;

        // start at the corresponding index
        let idx = (*virt >> (cfg::PAGE_BITS + level * cfg::LEVEL_BITS)) & cfg::LEVEL_MASK;
        let mut pte_addr = pt_virt + idx * util::size_of::<PTE>();

        while *pages > 0 {
            // reached end of page table?
            if pte_addr >= pt_virt + cfg::PAGE_SIZE {
                break;
            }

            let mut pte = unsafe { *(pte_addr as *const PTE) };

            // can we use a large page?
            if level == 0
                || (level == 1
                    && math::is_aligned(*virt, cfg::LPAGE_SIZE)
                    && math::is_aligned(*phys, cfg::LPAGE_SIZE as goff)
                    && *pages * cfg::PAGE_SIZE >= cfg::LPAGE_SIZE)
            {
                let (psize, flags) = if level == 1 {
                    (cfg::LPAGE_SIZE, dtu::PTEFlags::LARGE.bits())
                }
                else {
                    (cfg::PAGE_SIZE, 0)
                };

                let new_pte = to_mmu_pte(*phys | perm.bits() | flags);

                // determine if we need to perform an TLB invalidate
                let rwx = dtu::PTEFlags::RWX.bits() as PTE;
                let downgrade = (pte & rwx) != 0 && ((pte & rwx) & (!new_pte & rwx)) != 0;
                if downgrade {
                    invalidate_page(*virt);
                }

                unsafe { *(pte_addr as *mut PTE) = new_pte };

                log!(
                    crate::LOG_MAP_DETAIL,
                    "VPE{}: lvl {} PTE for 0x{:0>16x}: 0x{:0>16x} (downgrade={})",
                    self.vpe,
                    level,
                    virt,
                    new_pte,
                    downgrade
                );

                *pages -= psize / cfg::PAGE_SIZE;
                *virt += psize;
                *phys += psize as goff;
            }
            else {
                // unmapping non-existing PTs is a noop
                if !(pte == 0 && perm.is_empty()) {
                    if pte == 0 {
                        pte = self.create_pt(*virt, pte_addr, level)?;
                    }

                    self.map_pages_rec(virt, phys, pages, perm, to_dtu_pte(pte), level - 1)?;
                }
            }

            pte_addr += util::size_of::<PTE>();
        }

        Ok(())
    }

    fn create_pt(&self, virt: usize, pte_addr: usize, level: usize) -> Result<PTE, Error> {
        let frame = (self.alloc_frame)(self.vpe);
        if frame == 0 {
            return Err(Error::new(Code::NoSpace));
        }
        Self::clear_pt((self.xlate_pt)(self.vpe, frame));

        // insert PTE
        let pte = to_mmu_pte(frame | dtu::PTEFlags::IRWX.bits());
        unsafe { *(pte_addr as *mut PTE) = pte };

        let pt_size = (1 << (cfg::LEVEL_BITS * level)) * cfg::PAGE_SIZE;
        let virt_base = virt as usize & !(pt_size - 1);
        log!(
            crate::LOG_MAP_DETAIL,
            "VPE{}: lvl {} PTE for 0x{:0>16x}: 0x{:0>16x}",
            self.vpe,
            level,
            virt_base,
            pte
        );

        Ok(pte)
    }

    fn clear_pt(mut pt_virt: usize) {
        for _ in 0..1 << cfg::LEVEL_BITS {
            unsafe { *(pt_virt as *mut PTE) = 0 };
            pt_virt += util::size_of::<PTE>();
        }
    }

    fn print_as_rec(
        &self,
        f: &mut fmt::Formatter<'_>,
        pt: dtu::PTE,
        mut virt: usize,
        level: usize,
    ) {
        let mut ptes = (self.xlate_pt)(self.vpe, pt);
        for _ in 0..1 << cfg::LEVEL_BITS {
            let pte = unsafe { *(ptes as *const PTE) };
            if pte != 0 {
                let w = (cfg::LEVEL_CNT - level - 1) * 2;
                writeln!(f, "{:w$}0x{:0>16x}: 0x{:0>16x}", "", virt, pte, w = w).ok();
                if level > 0 && (pte & MMUFlags::LARGE.bits()) == 0 {
                    let pt = phys_to_noc(pte as u64 & !cfg::PAGE_MASK as u64);
                    self.print_as_rec(f, pt, virt, level - 1);
                }
            }

            virt += 1 << (level as usize * cfg::LEVEL_BITS + cfg::PAGE_BITS);
            ptes += util::size_of::<PTE>();

            // don't enter the PTE area
            if virt >= 0x0800_0000_0000 {
                break;
            }
        }
    }
}

// TODO implement Drop to free the page tables

impl fmt::Debug for AddrSpace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        writeln!(f, "Address space @ 0x{:0>16x}:", noc_to_phys(self.root))?;
        self.print_as_rec(f, self.root, 0, cfg::LEVEL_CNT - 1);
        Ok(())
    }
}
