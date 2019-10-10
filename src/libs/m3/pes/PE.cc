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
    if(!(flags() & KEEP_CAP)) {
        try {
            VPE::self().resmng()->free_pe(sel());
        }
        catch(...) {
            // ignore
        }
    }
    flags(KEEP_CAP);
}

PE PE::alloc(const PEDesc &desc) {
    capsel_t sel = VPE::self().alloc_sel();
    PEDesc res = VPE::self().resmng()->alloc_pe(sel, desc);
    return PE(sel, res, 0);
}

}
