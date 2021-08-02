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

#include <m3/pes/PE.h>
#include <m3/session/ResMng.h>
#include <m3/Exception.h>
#include <m3/Syscalls.h>
#include <m3/pes/VPE.h>

namespace m3 {

PE::~PE() {
    if(_free) {
        try {
            VPE::self().resmng()->free_pe(sel());
        }
        catch(...) {
            // ignore
        }
    }
}

Reference<PE> PE::alloc(const char *name) {
    capsel_t sel = VPE::self().alloc_sel();
    PEDesc res = VPE::self().resmng()->alloc_pe(sel, name);
    return Reference<PE>(new PE(sel, res, KEEP_CAP, true));
}

Reference<PE> PE::derive(uint eps) {
    capsel_t sel = VPE::self().alloc_sel();
    Syscalls::derive_pe(this->sel(), sel, eps);
    return Reference<PE>(new PE(sel, desc(), 0, false));
}

uint PE::quota() const {
    return Syscalls::pe_quota(sel());
}

}
