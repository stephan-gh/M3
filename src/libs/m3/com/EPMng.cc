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

#include <m3/com/EPMng.h>
#include <m3/com/Gate.h>
#include <m3/com/RecvGate.h>
#include <m3/Exception.h>
#include <m3/Syscalls.h>
#include <m3/pes/VPE.h>

namespace m3 {

EPMng::EPMng(bool mux)
    : _eps(),
      _next_victim(1),
      _gates(mux ? new Gate*[EP_COUNT]() : nullptr) {
}

epid_t EPMng::alloc_ep() {
    for(epid_t ep = DTU::FIRST_FREE_EP; ep < EP_COUNT; ++ep) {
        if(is_ep_free(ep)) {
            // if it's our own EP, check if it's available
            if(_gates && is_in_use(ep))
                continue;

            // invalidate the EP if necessary and possible
            activate(ep, ObjCap::INVALID);
            if(_gates && _gates[ep]) {
                _gates[ep]->set_epid(Gate::UNBOUND);
                _gates[ep] = nullptr;
            }
            _eps |= static_cast<uint64_t>(1) << ep;
            return ep;
        }
    }

    throw MessageException("Unable to allocate endpoint", Errors::NO_SPACE);
}

void EPMng::free_ep(epid_t id) noexcept {
    _eps &= ~(static_cast<uint64_t>(1) << id);
}

void EPMng::switch_to(Gate *gate) {
    assert(_gates != nullptr);
    epid_t victim = select_victim();
    activate(victim, gate->sel());
    _gates[victim] = gate;
    gate->set_epid(victim);
}

void EPMng::remove(Gate *gate, bool invalidate) noexcept {
    epid_t epid = gate->ep();
    if(_gates != nullptr) {
        if(epid != Gate::UNBOUND && gate->sel() != ObjCap::INVALID) {
            assert(_gates[epid] == nullptr || _gates[epid] == gate);
            if(invalidate) {
                try {
                    // invalidate our endpoint to be able to reuse it for something else later
                    activate(epid, ObjCap::INVALID);
                }
                catch(...) {
                    // ignore errors here
                }
            }
            _gates[epid] = nullptr;
        }
    }
    gate->set_epid(Gate::UNBOUND);
}

void EPMng::reset(uint64_t eps) noexcept {
    if(!_gates)
        _gates = new Gate*[EP_COUNT]();
    else {
        for(int i = 0; i < EP_COUNT; ++i) {
            if(_gates[i])
                _gates[i]->set_epid(Gate::UNBOUND);
            _gates[i] = nullptr;
        }
    }
    _eps = eps;
}

bool EPMng::is_ep_free(epid_t id) const noexcept {
    return id >= DTU::FIRST_FREE_EP && (_eps & (static_cast<uint64_t>(1) << id)) == 0;
}

bool EPMng::is_in_use(epid_t ep) const noexcept {
    return _gates[ep] && _gates[ep]->type() == ObjCap::SEND_GATE &&
           DTU::get().has_missing_credits(ep);
}

epid_t EPMng::select_victim() {
    epid_t victim = _next_victim;
    for(size_t count = 0; count < EP_COUNT; ++count) {
        if(!is_ep_free(victim) || is_in_use(victim))
            victim = (victim + 1) % EP_COUNT;
        else
            goto done;
    }
    throw MessageException("No free endpoints for multiplexing");

done:
    if(_gates[victim] != nullptr)
        _gates[victim]->set_epid(Gate::UNBOUND);

    _next_victim = (victim + 1) % EP_COUNT;
    return victim;
}

void EPMng::activate(epid_t ep, capsel_t newcap) {
    Syscalls::activate(EP::sel_of(ep), newcap, 0);
}

}
