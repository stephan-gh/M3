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

#include <base/Config.h>
#include <base/util/Math.h>
#include <base/log/Kernel.h>

#include "pes/VPE.h"
#include "Paging.h"
#include "Platform.h"

namespace kernel {

typedef goff_t (*alloc_frame_func)(uint64_t vpe);
typedef uintptr_t (*xlate_pt_func)(uint64_t vpe, goff_t phys);

extern "C" void *_text_start;
extern "C" void *_text_end;
extern "C" void *_data_start;
extern "C" void *_data_end;
extern "C" void *_bss_start;
extern "C" void *_bss_end;

extern "C" goff_t get_addr_space();
extern "C" void set_addr_space(goff_t root, alloc_frame_func alloc_frame, xlate_pt_func xlate_pt);
extern "C" uint64_t noc_to_phys(uint64_t noc);
extern "C" uint64_t phys_to_noc(uint64_t phys);
extern "C" void enable_paging();
extern "C" void init_aspace(uint64_t vpe,
                            alloc_frame_func alloc_frame, xlate_pt_func xlate_pt, goff_t root);
extern "C" void map_pages(uint64_t vpe, uintptr_t virt, goff_t phys, size_t pages, uint64_t perm,
                          alloc_frame_func alloc_frame, xlate_pt_func xlate_pt, goff_t root);
extern "C" uint64_t translate(uint64_t vpe, goff_t root, alloc_frame_func alloc_frame,
                              xlate_pt_func xlate_pt, uintptr_t virt, uint64_t perm);

static goff_t kalloc_frame(uint64_t) {
    static size_t pos = m3::env()->pe_mem_size / 2 + PAGE_SIZE;
    goff_t phys_begin = noc_to_phys(m3::env()->pe_mem_base);
    pos += PAGE_SIZE;
    return phys_begin + pos - PAGE_SIZE;
}

static uintptr_t kxlate_pt(uint64_t vpe, goff_t phys) {
    goff_t phys_begin = noc_to_phys(m3::env()->pe_mem_base);
    goff_t off = phys - phys_begin;
    return vpe == 0 ? off : PE_MEM_BASE + off;
}

static void map_init(uintptr_t virt, goff_t phys, size_t pages, uint64_t perm, goff_t root) {
    map_pages(0, virt, phys, pages, perm, kalloc_frame, kxlate_pt, root);
}

static void map_segment(void *start, void *end, uint64_t perm, goff_t root) {
    uintptr_t start_addr = m3::Math::round_dn(reinterpret_cast<uintptr_t>(start), PAGE_SIZE);
    uintptr_t end_addr = m3::Math::round_up(reinterpret_cast<uintptr_t>(end), PAGE_SIZE);
    size_t pages = (end_addr - start_addr) / PAGE_SIZE;
    map_init(start_addr, phys_to_noc(m3::env()->pe_mem_base + start_addr), pages, perm, root);
}

void init_paging() {
    if(!m3::env()->pedesc.has_virtmem())
        return;

    goff_t root = m3::env()->pe_mem_base + m3::env()->pe_mem_size / 2;
    init_aspace(0, kalloc_frame, kxlate_pt, root);

    // map TCU
    const uint64_t rw = m3::KIF::PageFlags::RW;
    map_init(m3::TCU::MMIO_ADDR, m3::TCU::MMIO_ADDR, m3::TCU::MMIO_SIZE / PAGE_SIZE, rw, root);
    map_init(m3::TCU::MMIO_PRIV_ADDR, m3::TCU::MMIO_PRIV_ADDR,
             m3::TCU::MMIO_PRIV_SIZE / PAGE_SIZE, m3::KIF::PageFlags::RW, root);

    // map text, data, and bss
    map_segment(&_text_start, &_text_end, m3::KIF::PageFlags::RX, root);
    map_segment(&_data_start, &_data_end, rw, root);
    map_segment(&_bss_start, &_bss_end, rw, root);

    // map initial heap
    uintptr_t heap_start = m3::Math::round_up(reinterpret_cast<uintptr_t>(&_bss_end), LPAGE_SIZE);
    map_init(heap_start, phys_to_noc(m3::env()->pe_mem_base + heap_start), 4, rw, root);

    // map stack
    map_init(STACK_BOTTOM, phys_to_noc(m3::env()->pe_mem_base + STACK_BOTTOM),
             STACK_SIZE / PAGE_SIZE, rw, root);
    // map env
    map_init(ENV_START, phys_to_noc(m3::env()->pe_mem_base + ENV_START),
             ENV_SIZE / PAGE_SIZE, rw, root);

    // map PTs
    map_init(PE_MEM_BASE, m3::env()->pe_mem_base, m3::env()->pe_mem_size / PAGE_SIZE, rw, root);

#if defined(__arm__)
    // map vectors
    map_init(0, m3::env()->pe_mem_base, 1, m3::KIF::PageFlags::RX, root);
#endif

    // switch to that address space
    set_addr_space(root, kalloc_frame, kxlate_pt);

    enable_paging();
}

void map_pages(uintptr_t virt, goff_t phys, size_t pages, uint64_t perm) {
    goff_t root = get_addr_space();
    map_pages(VPE::KERNEL_ID, virt, phys, pages, perm, kalloc_frame, kxlate_pt, root);
}

uint64_t translate(uintptr_t virt, uint64_t perm) {
    goff_t root = get_addr_space();
    return translate(VPE::KERNEL_ID, root, kalloc_frame, kxlate_pt, virt, perm);
}

}
