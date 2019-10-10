/*
 * Copyright (C) 2016-2017, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/Panic.h>

#include <m3/com/RecvGate.h>
#include <m3/Exception.h>
#include <m3/pes/VPE.h>

namespace m3 {

void *RecvGate::allocate(VPE &vpe, epid_t, size_t size) {
    uint64_t *cur = &vpe._rbufcur;
    uint64_t *end = &vpe._rbufend;

    if(*end == 0) {
        *cur = SYSC_RBUF_SIZE + UPCALL_RBUF_SIZE + DEF_RBUF_SIZE;
        *end = RECVBUF_SIZE;
    }

    // TODO atm, the kernel allocates the complete receive buffer space
    size_t left = *end - *cur;
    if(size > left)
        VTHROW(Errors::NO_SPACE, "Insufficient rbuf space for " << size << "b (" << left << "b left)");

    uint8_t *res = reinterpret_cast<uint8_t*>(*cur);
    *cur += size;
    return res;
}

void RecvGate::free(void *) {
    // TODO implement me
}

}
