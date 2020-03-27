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

#include <base/Env.h>
#include <base/Heap.h>
#include <base/KIF.h>
#include <base/stream/Serial.h>
#include <base/util/Math.h>

#include "mem/MainMemory.h"
#include "Paging.h"
#include "Platform.h"

namespace kernel {

class Gem5KEnvBackend : public m3::Gem5EnvBackend {
public:
    explicit Gem5KEnvBackend() {
    }

    virtual void init() override {
        init_rust_io(m3::env()->pe_id, "kernel");
        m3::Serial::init("kernel", m3::env()->pe_id);
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
        gaddr_t phys = m3::TCU::build_gaddr(alloc.pe(), alloc.addr);
        map_pages(virt, phys, pages, m3::KIF::PageFlags::RW);

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
    e->backend_addr = reinterpret_cast<uint64_t>(new Gem5KEnvBackend());
}

}