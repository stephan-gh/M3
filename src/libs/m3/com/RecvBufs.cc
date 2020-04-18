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

#include <base/Init.h>

#include <m3/com/RecvBufs.h>
#include <m3/Exception.h>
#include <m3/Syscalls.h>

namespace m3 {

INIT_PRIO_VFS RecvBufs RecvBufs::_inst;

RecvBuf *RecvBufs::alloc(size_t size) {
    bool vm = VPE::self().pe_desc().has_virtmem();
    // page align the receive buffers so that we can map them
    uintptr_t addr = _bufs.allocate(size, vm ? PAGE_SIZE : 1);
    if(addr == 0)
        VTHROW(Errors::NO_SPACE, "Insufficient rbuf space for " << size << "b");

    std::unique_ptr<MemGate> mgate;
    if(vm) {
        // allocate memory
        size_t aligned_size = Math::round_up(size, PAGE_SIZE);
        mgate.reset(new MemGate(MemGate::create_global(aligned_size, MemGate::R)));

        // map receive buffer
        capsel_t dst = addr / PAGE_SIZE;
        capsel_t pages = aligned_size / PAGE_SIZE;
        try {
            Syscalls::create_map(dst, VPE::self().sel(), mgate->sel(), 0, pages, MemGate::R);
        }
        catch(...) {
            // undo allocation
            _bufs.free(addr, size);
            throw;
        }
    }

    return new RecvBuf(addr, size, mgate);
}

void RecvBufs::free(RecvBuf *rbuf) noexcept {
    _bufs.free(rbuf->addr(), rbuf->size());
    delete rbuf;
}

}
