/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

#include <base/Errors.h>
#include <base/util/Util.h>

#include <m3/Exception.h>
#include <m3/Syscalls.h>
#include <m3/com/MemGate.h>
#include <m3/session/ResMng.h>
#include <m3/tiles/Activity.h>

#include <assert.h>
#include <thread/ThreadManager.h>

namespace m3 {

static bool destruct(capsel_t sel, uint flags, bool resmng) {
    if(!(flags & ObjCap::KEEP_CAP) && resmng) {
        try {
            Activity::own().resmng()->free_mem(sel);
        }
        catch(...) {
            // ignore
        }
        return true;
    }
    return false;
}

MemCap MemCap::create_global(size_t size, int perms, capsel_t sel) {
    if(sel == INVALID)
        sel = SelSpace::get().alloc_sel();
    Activity::own().resmng()->alloc_mem(sel, size, perms);
    return MemCap(0, sel, true);
}

MemCap MemCap::bind_bootmod(const std::string_view &name) {
    auto sel = SelSpace::get().alloc_sel();
    Activity::own().resmng()->use_mod(sel, name);
    return MemCap(0, sel, false);
}

MemCap MemCap::derive(goff_t offset, size_t size, int perms) const {
    capsel_t nsel = SelSpace::get().alloc_sel();
    Syscalls::derive_mem(Activity::own().sel(), nsel, sel(), offset, size, perms);
    return MemCap(0, nsel, false);
}

MemCap MemCap::derive_for(capsel_t act, capsel_t cap, goff_t offset, size_t size, int perms) const {
    Syscalls::derive_mem(act, cap, sel(), offset, size, perms);
    return MemCap(0, cap, false);
}

MemGate MemCap::activate() {
    auto org_flags = flags();

    EP *ep = Gate::activate(sel());

    // don't revoke the cap
    flags(KEEP_CAP);

    return MemGate(org_flags, sel(), _resmng, ep);
}

void MemCap::activate_on(const EP &ep) {
    Gate::activate_on(sel(), ep);
}

MemCap::~MemCap() {
    if(destruct(sel(), flags(), _resmng))
        flags(KEEP_CAP);
}

MemGate::~MemGate() {
    if(destruct(sel(), flags(), _resmng))
        flags(KEEP_CAP);
}

void MemGate::read(void *data, size_t len, goff_t offset) {
    Errors::Code res = TCU::get().read(ep()->id(), data, len, offset);
    if(EXPECT_FALSE(res != Errors::SUCCESS))
        throw TCUException(res);
}

void MemGate::write(const void *data, size_t len, goff_t offset) {
    Errors::Code res = TCU::get().write(ep()->id(), data, len, offset);
    if(EXPECT_FALSE(res != Errors::SUCCESS))
        throw TCUException(res);
}

}
