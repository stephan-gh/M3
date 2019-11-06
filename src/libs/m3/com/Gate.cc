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

#include <m3/com/Gate.h>
#include <m3/pes/VPE.h>
#include <m3/Syscalls.h>

namespace m3 {

Gate::~Gate() {
    release_ep(VPE::self());
}

const EP &Gate::acquire_ep() {
    if(!_ep)
        _ep = VPE::self().epmng().acquire();
    return *_ep;
}

const EP &Gate::activate(uintptr_t addr) {
    if(!_ep) {
        _ep = VPE::self().epmng().acquire();
        activate_on(*_ep, addr);
    }
    return *_ep;
}

void Gate::activate_on(const EP &ep, uintptr_t addr) {
    Syscalls::activate(ep.sel(), sel(), addr);
}

void Gate::release_ep(VPE &vpe) noexcept {
    if(_ep && _ep->id() >= DTU::FIRST_FREE_EP) {
        vpe.epmng().release(_ep, flags() & KEEP_CAP);
        _ep = nullptr;
    }
}

}
