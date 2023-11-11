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

#include <m3/Syscalls.h>
#include <m3/tiles/KMem.h>
#include <m3/tiles/OwnActivity.h>

namespace m3 {

Quota<size_t> KMem::quota() const {
    return Syscalls::kmem_quota(sel());
}

Reference<KMem> KMem::derive(const KMem &base, size_t quota) {
    capsel_t sel = SelSpace::get().alloc_sel();
    Syscalls::derive_kmem(base.sel(), sel, quota);
    return Reference<KMem>(new KMem(sel, 0));
}

}
