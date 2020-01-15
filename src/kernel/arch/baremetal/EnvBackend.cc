/*
 * Copyright (C) 2016-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/Common.h>
#include <base/util/Math.h>
#include <base/Env.h>
#include <base/Heap.h>

#include "mem/MainMemory.h"
#include "pes/VPEManager.h"
#include "pes/VPE.h"
#include "DTU.h"
#include "Platform.h"
#include "WorkLoop.h"

typedef goff_t (*alloc_frame_func)(uint64_t vpe);
typedef uintptr_t (*xlate_pt_func)(uint64_t vpe, goff_t phys);

extern "C" void init_rust_io(uint pe_id, const char *name);
extern "C" goff_t get_addr_space();
extern "C" void map_pages(uint64_t vpe, uintptr_t virt, goff_t phys, size_t pages, uint64_t perm,
                          alloc_frame_func alloc_frame, xlate_pt_func xlate_pt, goff_t root);

namespace kernel {

static goff_t kalloc_frame(uint64_t) {
    static size_t pos = Platform::pe_mem_size() / 2;
    goff_t phys_begin = Platform::pe_mem_base();
    pos += PAGE_SIZE;
    return phys_begin + pos - PAGE_SIZE;
}

static uintptr_t kxlate_pt(uint64_t, goff_t phys) {
    goff_t phys_begin = Platform::pe_mem_base();
    goff_t off = phys - phys_begin;
    return PE_MEM_BASE + off;
}

class BaremetalKEnvBackend : public m3::BaremetalEnvBackend {
public:
    explicit BaremetalKEnvBackend() {
    }

    virtual void init() override {
        init_rust_io(m3::env()->pe, "kernel");
        m3::Serial::init("kernel", m3::env()->pe);
    }

    virtual void reinit() override {
        // not used
    }

    virtual bool extend_heap(size_t size) override {
        if(!Platform::pe(Platform::kernel_pe()).has_virtmem())
            return false;

        uint pages = m3::Math::max((size_t)8,
            m3::Math::round_up<size_t>(size, PAGE_SIZE) >> PAGE_BITS);

        // allocate memory
        MainMemory::Allocation alloc = MainMemory::get().allocate(pages * PAGE_SIZE, PAGE_SIZE);
        if(!alloc)
            return false;

        // map the memory
        uintptr_t virt = m3::Math::round_up<uintptr_t>(
            reinterpret_cast<uintptr_t>(heap_end), PAGE_SIZE);
        gaddr_t phys = m3::DTU::build_gaddr(alloc.pe(), alloc.addr);

        goff_t root = get_addr_space();
        ::map_pages(VPE::KERNEL_ID, virt, phys, pages, m3::DTU::PTE_I | m3::DTU::PTE_RW,
                    kalloc_frame, kxlate_pt, root);

        // ensure that Heap::append is not done before all PTEs have been created
        m3::CPU::memory_barrier();

        m3::Heap::append(pages);
        return true;
    }

    virtual void exit(int) override {
        m3::Machine::shutdown();
    }
};

EXTERN_C void init_env(m3::Env *e) {
    m3::Heap::init();
    e->_backend = reinterpret_cast<uint64_t>(new BaremetalKEnvBackend());
}

}
