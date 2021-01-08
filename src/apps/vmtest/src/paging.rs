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

use base::cell::{LazyStaticCell, StaticCell};
use base::cfg;
use base::envdata;
use base::errors::Error;
use base::goff;
use base::kif::{PEDesc, PageFlags, PTE};
use base::math;
use base::mem::GlobAddr;
use base::tcu;

use paging::{self, AddrSpace, Allocator, Phys};

extern "C" {
    static _text_start: u8;
    static _text_end: u8;
    static _data_start: u8;
    static _data_end: u8;
    static _bss_start: u8;
    static _bss_end: u8;
}

struct PTAllocator {}

impl Allocator for PTAllocator {
    fn allocate_pt(&mut self) -> Result<Phys, Error> {
        PT_POS.set(*PT_POS + cfg::PAGE_SIZE as goff);
        Ok(*PT_POS - cfg::PAGE_SIZE as goff)
    }

    fn translate_pt(&self, phys: Phys) -> usize {
        let phys_begin = paging::glob_to_phys(envdata::get().pe_mem_base);
        let off = (phys - phys_begin) as usize;
        if *BOOTSTRAP {
            phys as usize
        }
        else {
            cfg::PE_MEM_BASE + off
        }
    }

    fn free_pt(&mut self, _phys: Phys) {
        unimplemented!();
    }
}

static BOOTSTRAP: StaticCell<bool> = StaticCell::new(true);
static PT_POS: LazyStaticCell<goff> = LazyStaticCell::default();
static ASPACE: LazyStaticCell<AddrSpace<PTAllocator>> = LazyStaticCell::default();

pub fn init() {
    assert!(PEDesc::new_from(envdata::get().pe_desc).has_virtmem());

    let root = envdata::get().pe_mem_base;
    PT_POS.set(root + cfg::PAGE_SIZE as goff);
    let mut aspace = AddrSpace::new(0, GlobAddr::new(root), PTAllocator {});
    aspace.init();

    // map TCU
    let rw = PageFlags::RW;
    map_ident(&mut aspace, tcu::MMIO_ADDR, tcu::MMIO_SIZE, rw);
    map_ident(&mut aspace, tcu::MMIO_PRIV_ADDR, tcu::MMIO_PRIV_SIZE, rw);

    // map text, data, and bss
    unsafe {
        map_segment(&mut aspace, &_text_start, &_text_end, PageFlags::RX);
        map_segment(&mut aspace, &_data_start, &_data_end, PageFlags::RW);
        map_segment(&mut aspace, &_bss_start, &_bss_end, PageFlags::RW);

        // map initial heap
        let heap_start = math::round_up(&_bss_end as *const _ as usize, cfg::PAGE_SIZE);
        map_ident(&mut aspace, heap_start, 4 * cfg::PAGE_SIZE, rw);
    }

    // map env
    map_ident(&mut aspace, cfg::ENV_START, cfg::ENV_SIZE, rw);

    // map PTs
    let glob = GlobAddr::new(envdata::get().pe_mem_base);
    let pages = envdata::get().pe_mem_size as usize / cfg::PAGE_SIZE;
    aspace.map_pages(cfg::PE_MEM_BASE, glob, pages, rw).unwrap();

    // switch to that address space
    aspace.switch_to();
    paging::enable_paging();

    ASPACE.set(aspace);
    BOOTSTRAP.set(false);
}

pub fn translate(virt: usize, perm: PageFlags) -> PTE {
    ASPACE.translate(virt, perm.bits())
}

pub fn map_anon(virt: usize, size: usize, perm: PageFlags) -> Result<(), Error> {
    for i in 0..(size / cfg::PAGE_SIZE) {
        let frame = ASPACE.get_mut().allocator_mut().allocate_pt()?;
        ASPACE.get_mut().map_pages(
            virt + i * cfg::PAGE_SIZE,
            GlobAddr::new(paging::phys_to_glob(frame)),
            1,
            perm,
        )?;
    }
    Ok(())
}

fn map_segment(
    aspace: &mut AddrSpace<PTAllocator>,
    start: *const u8,
    end: *const u8,
    perm: PageFlags,
) {
    let start_addr = math::round_dn(start as usize, cfg::PAGE_SIZE);
    let end_addr = math::round_up(end as usize, cfg::PAGE_SIZE);
    map_ident(aspace, start_addr, end_addr - start_addr, perm);
}

fn map_ident(aspace: &mut AddrSpace<PTAllocator>, virt: usize, size: usize, perm: PageFlags) {
    let glob = GlobAddr::new(virt as goff);
    aspace
        .map_pages(virt, glob, size / cfg::PAGE_SIZE, perm)
        .unwrap();
}
