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

#include "pes/VPE.h"
#include "Paging.h"
#include "Platform.h"

namespace kernel {

typedef goff_t (*alloc_frame_func)(uint64_t vpe);
typedef uintptr_t (*xlate_pt_func)(uint64_t vpe, goff_t phys);

extern "C" goff_t get_addr_space();
extern "C" uint64_t noc_to_phys(uint64_t noc);
extern "C" void map_pages(uint64_t vpe, uintptr_t virt, goff_t phys, size_t pages, uint64_t perm,
                          alloc_frame_func alloc_frame, xlate_pt_func xlate_pt, goff_t root);

static goff_t kalloc_frame(uint64_t) {
    static size_t pos = Platform::pe_mem_size() / 2;
    goff_t phys_begin = noc_to_phys(Platform::pe_mem_base());
    pos += PAGE_SIZE;
    return phys_begin + pos - PAGE_SIZE;
}

static uintptr_t kxlate_pt(uint64_t, goff_t phys) {
    goff_t phys_begin = noc_to_phys(Platform::pe_mem_base());
    goff_t off = phys - phys_begin;
    return PE_MEM_BASE + off;
}

void map_pages(uintptr_t virt, goff_t phys, size_t pages, uint64_t perm) {
    goff_t root = get_addr_space();
    map_pages(VPE::KERNEL_ID, virt, phys, pages, perm, kalloc_frame, kxlate_pt, root);
}

}
