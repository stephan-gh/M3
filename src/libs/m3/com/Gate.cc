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
#include <m3/DTUIf.h>
#include <m3/VPE.h>

namespace m3 {

Gate::~Gate() {
    if(ep() != UNBOUND && ep() >= DTU::FIRST_FREE_EP)
        DTUIf::remove_gate(*this, flags() & KEEP_CAP);
}

void Gate::put_ep(EP &&ep) noexcept {
    if(ep.id() >= DTU::FIRST_FREE_EP)
        ep.assign(*this);
    _ep = std::move(ep);
}

epid_t Gate::acquire_ep() {
    if(ep() == UNBOUND && sel() != ObjCap::INVALID)
        VPE::self().epmng().switch_to(this);
    return ep();
}

}
