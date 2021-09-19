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

#include <m3/pes/KMem.h>
#include <m3/pes/VPE.h>
#include <m3/Syscalls.h>

namespace m3 {

size_t KMem::quota() const {
    return Syscalls::kmem_quota(sel(), nullptr);
}

Reference<KMem> KMem::derive(const KMem &base, size_t quota) {
    capsel_t sel = VPE::self().alloc_sel();
    Syscalls::derive_kmem(base.sel(), sel, quota);
    return Reference<KMem>(new KMem(sel, 0));
}

}
