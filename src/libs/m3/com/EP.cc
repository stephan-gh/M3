/*
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

#include <m3/Exception.h>
#include <m3/Syscalls.h>
#include <m3/com/EP.h>
#include <m3/tiles/OwnActivity.h>

namespace m3 {

EP::EP() noexcept : EP(ObjCap::INVALID, Gate::UNBOUND, 0, KEEP_CAP) {
}

EP &EP::operator=(EP &&ep) noexcept {
    release();
    sel(ep.sel());
    flags(ep.flags());
    _id = ep._id;
    _replies = ep._replies;
    ep.flags(KEEP_CAP);
    return *this;
}

EP EP::alloc(uint replies) {
    return alloc_for(KIF::SEL_ACT, TOTAL_EPS, replies);
}

EP EP::alloc_for(capsel_t act, epid_t ep, uint replies) {
    capsel_t sel = SelSpace::get().alloc_sel();
    epid_t id = Syscalls::alloc_ep(sel, act, ep, replies);
    return EP(sel, id, replies, 0);
}

EP EP::bind(epid_t id) noexcept {
    return EP(ObjCap::INVALID, id, 0, KEEP_CAP);
}

}
