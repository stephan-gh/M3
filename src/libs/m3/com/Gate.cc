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

SList<Gate> Gate::_gates;

Gate::~Gate() {
    release_ep(VPE::self());
}

const EP &Gate::acquire_ep() {
    if(!_ep)
        _ep = VPE::self().epmng().acquire();
    return *_ep;
}

const EP &Gate::activate(capsel_t rbuf_mem, goff_t rbuf_off) {
    if(!_ep) {
        _ep = VPE::self().epmng().acquire();
        activate_on(*_ep, rbuf_mem, rbuf_off);
    }
    return *_ep;
}

void Gate::activate_on(const EP &ep, capsel_t rbuf_mem, goff_t rbuf_off) {
    Syscalls::activate(ep.sel(), sel(), rbuf_mem, rbuf_off);
    _gates.append(this);
}

void Gate::deactivate() {
    release_ep(VPE::self(), true);
}

void Gate::release_ep(VPE &vpe, bool force_inval) noexcept {
    if(_ep && !_ep->is_standard()) {
        vpe.epmng().release(_ep, force_inval || (flags() & KEEP_CAP));
        _gates.remove(this);
        _ep = nullptr;
    }
}

void Gate::reset() {
    for(auto g = _gates.begin(); g != _gates.end(); ++g)
        g->_ep = nullptr;
    _gates.clear();
}

}
