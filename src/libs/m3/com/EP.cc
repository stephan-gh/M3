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

#include <m3/com/EP.h>
#include <m3/Exception.h>
#include <m3/Syscalls.h>
#include <m3/pes/VPE.h>

namespace m3 {

EP::EP() noexcept
    : EP(ObjCap::INVALID, Gate::UNBOUND, false, KEEP_CAP) {
}

EP &EP::operator=(EP &&ep) noexcept {
    release();
    free_ep();
    sel(ep.sel());
    flags(ep.flags());
    _id = ep._id;
    _free = ep._free;
    ep._free = false;
    ep.flags(KEEP_CAP);
    return *this;
}

EP::~EP() {
    free_ep();
}

void EP::free_ep() {
    if(_free)
        VPE::self().epmng().free_ep(_id);
}

capsel_t EP::sel_of(epid_t ep) noexcept {
    return KIF::FIRST_EP_SEL + ep - DTU::FIRST_FREE_EP;
}

capsel_t EP::sel_of_vpe(VPE &vpe, epid_t ep) noexcept {
    static_assert(KIF::SEL_PE == 0, "PE selector changed");
    return vpe.pe().sel() + sel_of(ep);
}

EP EP::alloc() {
    return alloc_for(VPE::self());
}

EP EP::alloc_for(VPE &vpe) {
    if(env()->shared) {
        epid_t id;
        capsel_t sel = alloc_cap(vpe, &id);
        return EP(sel, id, false, 0);
    }

    epid_t id = vpe.epmng().alloc_ep();
    return EP(sel_of_vpe(vpe, id), id, true, KEEP_CAP);
}

EP EP::bind(epid_t id) noexcept {
    capsel_t sel = id == Gate::UNBOUND ? ObjCap::INVALID : sel_of(id);
    return EP(sel, id, false, KEEP_CAP);
}

EP EP::bind_for(VPE &vpe, epid_t id) noexcept {
    capsel_t sel = id == Gate::UNBOUND ? ObjCap::INVALID : sel_of_vpe(vpe, id);
    return EP(sel, id, false, KEEP_CAP);
}

capsel_t EP::alloc_cap(VPE &vpe, epid_t *id) {
    capsel_t sel = VPE::self().alloc_sel();
    *id = Syscalls::alloc_ep(sel, vpe.sel(), vpe.pe().sel());
    return sel;
}

void EP::assign(Gate &gate) {
    DTUIf::switch_gate(*this, gate);
}

}
