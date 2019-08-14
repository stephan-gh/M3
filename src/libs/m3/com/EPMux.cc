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

#include <base/Errors.h>
#include <base/Init.h>
#include <base/Panic.h>

#include <m3/com/EPMux.h>
#include <m3/com/Gate.h>
#include <m3/com/RecvGate.h>
#include <m3/Exception.h>
#include <m3/Syscalls.h>
#include <m3/VPE.h>

namespace m3 {

INIT_PRIO_EPMUX EPMux EPMux::_inst;

EPMux::EPMux()
    : _next_victim(1),
      _gates() {
}

bool EPMux::reserve(epid_t ep) {
    // take care that some non-fixed gate could already use that endpoint
    if(is_in_use(ep))
        return false;

    if(_gates[ep]) {
        activate(ep, ObjCap::INVALID);
        _gates[ep]->_ep = Gate::UNBOUND;
        _gates[ep] = nullptr;
    }
    return true;
}

void EPMux::switch_to(Gate *gate) {
    epid_t victim = select_victim();
    activate(victim, gate->sel());
    _gates[victim] = gate;
    gate->_ep = victim;
}

void EPMux::remove(Gate *gate, bool invalidate) noexcept {
    if(gate->_ep != Gate::NODESTROY && gate->_ep != Gate::UNBOUND && gate->sel() != ObjCap::INVALID) {
        assert(_gates[gate->_ep] == nullptr || _gates[gate->_ep] == gate);
        if(invalidate) {
            try {
                // we have to invalidate our endpoint, i.e. set the registers to zero. otherwise the cmpxchg
                // will fail when we program the next gate on this endpoint.
                // note that the kernel has to validate that it is 0 for "unused endpoints" because otherwise
                // we could just specify that our endpoint is unused and the kernel won't check it and thereby
                // trick the whole system.
                activate(gate->_ep, ObjCap::INVALID);
            }
            catch(...) {
                // ignore errors here
            }
        }
        _gates[gate->_ep] = nullptr;
        gate->_ep = Gate::UNBOUND;
    }
}

void EPMux::reset() noexcept {
    for(int i = 0; i < EP_COUNT; ++i) {
        if(_gates[i])
            _gates[i]->_ep = Gate::UNBOUND;
        _gates[i] = nullptr;
    }
}

bool EPMux::is_in_use(epid_t ep) const {
    return _gates[ep] && _gates[ep]->type() == ObjCap::SEND_GATE &&
           DTU::get().has_missing_credits(ep);
}

epid_t EPMux::select_victim() {
    epid_t victim = _next_victim;
    for(size_t count = 0; count < EP_COUNT; ++count) {
        if(!VPE::self().is_ep_free(victim) || is_in_use(victim))
            victim = (victim + 1) % EP_COUNT;
        else
            goto done;
    }
    throw MessageException("No free endpoints for multiplexing");

done:
    if(_gates[victim] != nullptr)
        _gates[victim]->_ep = Gate::UNBOUND;

    _next_victim = (victim + 1) % EP_COUNT;
    return victim;
}

void EPMux::activate(epid_t ep, capsel_t newcap) {
    Syscalls::activate(VPE::self().ep_to_sel(ep), newcap, 0);
}

}
