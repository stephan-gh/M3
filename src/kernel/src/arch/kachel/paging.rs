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
use base::mem::{heap, GlobAddr};
use base::tcu;
use core::cmp;

use crate::mem;
use crate::paging::{self, AddrSpace, Allocator, Phys};
use crate::pes;
use crate::platform;

extern "C" {
    fn heap_set_oom_callback(cb: extern "C" fn(size: usize) -> bool);

    static mut heap_end: *mut heap::HeapArea;

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
        if BOOTSTRAP.get() {
            phys as usize
        }
        else {
            cfg::PE_MEM_BASE + (phys as usize - cfg::MEM_OFFSET)
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
    unsafe {
        heap_set_oom_callback(kernel_oom_callback);
    }

    if !PEDesc::new_from(envdata::get().pe_desc).has_virtmem() {
        paging::disable_paging();
        return;
    }

    let (mem_pe, mem_base, mem_size, _) = tcu::TCU::unpack_mem_ep(0).unwrap();

    let base = GlobAddr::new_with(mem_pe, mem_base);
    let root = base + mem_size / 2;
    let pts_phys = cfg::MEM_OFFSET as goff + mem_size / 2;
    PT_POS.set(pts_phys + cfg::PAGE_SIZE as goff);
    let mut aspace = AddrSpace::new(pes::KERNEL_ID as u64, root, PTAllocator {});
    aspace.init();

    // map TCU
    let rw = PageFlags::RW;
    map_ident(&mut aspace, tcu::MMIO_ADDR, tcu::MMIO_SIZE, rw);
    map_ident(&mut aspace, tcu::MMIO_PRIV_ADDR, tcu::MMIO_PRIV_SIZE, rw);

    // map text, data, and bss
    unsafe {
        map_segment(&mut aspace, base, &_text_start, &_text_end, PageFlags::RX);
        map_segment(&mut aspace, base, &_data_start, &_data_end, PageFlags::RW);
        map_segment(&mut aspace, base, &_bss_start, &_bss_end, PageFlags::RW);

        // map initial heap
        let heap_start = math::round_up(&_bss_end as *const _ as usize, cfg::PAGE_SIZE);
        map_to_phys(
            &mut aspace,
            base,
            heap_start,
            envdata::get().heap_size as usize,
            rw,
        );
    }

    // map env
    map_to_phys(
        &mut aspace,
        base,
        cfg::ENV_START & !cfg::PAGE_MASK,
        cfg::ENV_SIZE,
        rw,
    );

    // map PTs
    let pages = mem_size as usize / cfg::PAGE_SIZE;
    aspace.map_pages(cfg::PE_MEM_BASE, base, pages, rw).unwrap();

    // map vectors
    #[cfg(target_arch = "arm")]
    map_to_phys(&mut aspace, base, 0, cfg::PAGE_SIZE, PageFlags::RX);

    // switch to that address space
    aspace.switch_to();
    paging::enable_paging();

    ASPACE.set(aspace);
    BOOTSTRAP.set(false);
}

pub fn translate(virt: usize, perm: PageFlags) -> PTE {
    ASPACE.translate(virt, perm.bits())
}

pub fn map_new_mem(virt: usize, pages: usize) -> GlobAddr {
    let mut alloc = mem::get()
        .allocate(
            mem::MemType::KERNEL,
            (pages * cfg::PAGE_SIZE) as goff,
            cfg::PAGE_SIZE as goff,
        )
        .unwrap();

    ASPACE
        .get_mut()
        .map_pages(virt, alloc.global(), pages, PageFlags::RW)
        .unwrap();
    alloc.claim();
    alloc.global()
}

fn map_ident(aspace: &mut AddrSpace<PTAllocator>, virt: usize, size: usize, perm: PageFlags) {
    let glob = GlobAddr::new(virt as goff);
    aspace
        .map_pages(virt, glob, size / cfg::PAGE_SIZE, perm)
        .unwrap();
}

fn map_to_phys(
    aspace: &mut AddrSpace<PTAllocator>,
    base: GlobAddr,
    virt: usize,
    size: usize,
    perm: PageFlags,
) {
    let glob = base + (virt - cfg::MEM_OFFSET) as Phys;
    aspace
        .map_pages(virt, glob, size / cfg::PAGE_SIZE, perm)
        .unwrap();
}

fn map_segment(
    aspace: &mut AddrSpace<PTAllocator>,
    base: GlobAddr,
    start: *const u8,
    end: *const u8,
    perm: PageFlags,
) {
    let start_addr = math::round_dn(start as usize, cfg::PAGE_SIZE);
    let end_addr = math::round_up(end as usize, cfg::PAGE_SIZE);
    map_to_phys(aspace, base, start_addr, end_addr - start_addr, perm);
}

extern "C" fn kernel_oom_callback(size: usize) -> bool {
    if !platform::pe_desc(platform::kernel_pe()).has_virtmem() {
        panic!(
            "Unable to allocate {} bytes on the heap: out of memory",
            size
        );
    }

    // allocate and map more physical memory
    let pages = cmp::max(256, math::round_up(size, cfg::PAGE_SIZE) >> cfg::PAGE_BITS);
    let virt = unsafe { math::round_up(heap_end as usize, cfg::PAGE_SIZE) };
    map_new_mem(virt, pages);

    // append to heap
    heap::append(pages);
    true
}
