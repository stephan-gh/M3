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

#include <base/util/Util.h>
#include <base/Errors.h>

#include <m3/com/MemGate.h>
#include <m3/session/ResMng.h>
#include <m3/Exception.h>
#include <m3/Syscalls.h>
#include <m3/VPE.h>

#include <thread/ThreadManager.h>

#include <assert.h>

namespace m3 {

MemGate::~MemGate() {
    if(!(flags() & KEEP_CAP) && !_revoke) {
        try {
            VPE::self().resmng()->free_mem(sel());
        }
        catch(...) {
            // ignore
        }
        flags(KEEP_CAP);
    }
}

MemGate MemGate::create_global_for(goff_t addr, size_t size, int perms, capsel_t sel, uint flags) {
    if(sel == INVALID)
        sel = VPE::self().alloc_sel();
    VPE::self().resmng()->alloc_mem(sel, addr, size, perms);
    return MemGate(flags, sel, false);
}

MemGate MemGate::derive(goff_t offset, size_t size, int perms) const {
    capsel_t nsel = VPE::self().alloc_sel();
    Syscalls::derive_mem(VPE::self().sel(), nsel, sel(), offset, size, perms);
    return MemGate(0, nsel, true);
}

MemGate MemGate::derive_for(capsel_t vpe, capsel_t cap, goff_t offset, size_t size, int perms, uint flags) const {
    Syscalls::derive_mem(vpe, cap, sel(), offset, size, perms);
    return MemGate(flags, cap, true);
}

void MemGate::activate_for(VPE &vpe, epid_t ep, goff_t offset) {
    Syscalls::activate(vpe.ep_to_sel(ep), sel(), offset);
    if(&vpe == &VPE::self())
        Gate::ep(ep);
}

Errors::Code MemGate::forward(void *&data, size_t &len, goff_t &offset, uint flags) {
    event_t event = ThreadManager::get().get_wait_event();
    size_t amount = Math::min(static_cast<size_t>(KIF::MAX_MSG_SIZE), len);
    Errors::Code res = Syscalls::forward_mem(sel(), data, amount, offset, flags, event);

    // if this has been done, go to sleep and wait until the kernel sends us the upcall
    if(res == Errors::UPCALL_REPLY) {
        ThreadManager::get().wait_for(event);
        auto *msg = reinterpret_cast<const KIF::Upcall::Forward*>(
            ThreadManager::get().get_current_msg());
        Errors::Code res = static_cast<Errors::Code>(msg->error);
        if(res != Errors::NONE && res != Errors::PAGEFAULT)
            throw SyscallException(res, KIF::Syscall::FORWARD_MEM);
    }

    if(res == Errors::NONE) {
        len -= amount;
        offset += amount;
        data = reinterpret_cast<void*>(reinterpret_cast<uintptr_t>(data) + amount);
    }
    return res;
}

void MemGate::read(void *data, size_t len, goff_t offset) {
    ensure_activated();

retry:
    Errors::Code res = DTU::get().read(ep(), data, len, offset, _cmdflags);
    if(EXPECT_FALSE(res == Errors::VPE_GONE)) {
        res = forward(data, len, offset, _cmdflags);
        if(len > 0 || res == Errors::PAGEFAULT)
            goto retry;
    }
    if(EXPECT_FALSE(res != Errors::NONE))
        throw DTUException(res);
}

void MemGate::write(const void *data, size_t len, goff_t offset) {
    ensure_activated();

retry:
    Errors::Code res = DTU::get().write(ep(), data, len, offset, _cmdflags);
    if(EXPECT_FALSE(res == Errors::VPE_GONE)) {
        res = forward(const_cast<void*&>(data), len, offset,
            _cmdflags | KIF::Syscall::ForwardMem::WRITE);
        if(len > 0 || res == Errors::PAGEFAULT)
            goto retry;
    }
    if(EXPECT_FALSE(res != Errors::NONE))
        throw DTUException(res);
}

}
