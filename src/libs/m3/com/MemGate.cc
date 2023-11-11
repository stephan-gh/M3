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

MemGate::~MemGate() {
    if(!(flags() & KEEP_CAP) && !_revoke) {
        try {
            Activity::own().resmng()->free_mem(sel());
        }
        catch(...) {
            // ignore
        }
        flags(KEEP_CAP);
    }
}

MemGate MemGate::create_global(size_t size, int perms, capsel_t sel, uint flags) {
    if(sel == INVALID)
        sel = SelSpace::get().alloc_sel();
    Activity::own().resmng()->alloc_mem(sel, size, perms);
    return MemGate(flags, sel, false);
}

MemGate MemGate::bind_bootmod(const std::string_view &name) {
    auto sel = SelSpace::get().alloc_sel();
    Activity::own().resmng()->use_mod(sel, name);
    return MemGate(0, sel, true);
}

MemGate MemGate::derive(goff_t offset, size_t size, int perms) const {
    capsel_t nsel = SelSpace::get().alloc_sel();
    Syscalls::derive_mem(Activity::own().sel(), nsel, sel(), offset, size, perms);
    return MemGate(0, nsel, true);
}

MemGate MemGate::derive_for(capsel_t act, capsel_t cap, goff_t offset, size_t size, int perms,
                            uint flags) const {
    Syscalls::derive_mem(act, cap, sel(), offset, size, perms);
    return MemGate(flags, cap, true);
}

void MemGate::read(void *data, size_t len, goff_t offset) {
    const EP &ep = activate();
    Errors::Code res = TCU::get().read(ep.id(), data, len, offset);
    if(EXPECT_FALSE(res != Errors::SUCCESS))
        throw TCUException(res);
}

void MemGate::write(const void *data, size_t len, goff_t offset) {
    const EP &ep = activate();
    Errors::Code res = TCU::get().write(ep.id(), data, len, offset);
    if(EXPECT_FALSE(res != Errors::SUCCESS))
        throw TCUException(res);
}

}
