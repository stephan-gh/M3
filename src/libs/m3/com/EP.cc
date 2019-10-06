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
#include <m3/session/ResMng.h>
#include <m3/Exception.h>
#include <m3/Syscalls.h>
#include <m3/VPE.h>

namespace m3 {

EP::EP() noexcept
    : EP(ObjCap::INVALID, Gate::UNBOUND, false) {
}

EP &EP::operator=(EP &&ep) noexcept {
    free_ep();
    sel(ep.sel());
    _id = ep._id;
    _free = ep._free;
    ep._free = false;
    return *this;
}

EP::~EP() {
    free_ep();
}

void EP::free_ep() {
    if(_free) {
        if(USE_PEXCALLS) {
            try {
                VPE::self().resmng()->free_ep(sel());
            }
            catch(...) {
                // ignore
            }
        }
        else
            VPE::self().epmng().free_ep(_id);
    }
}

capsel_t EP::sel_of(VPE &vpe, epid_t ep) noexcept {
    return vpe.sel() + KIF::FIRST_EP_SEL + ep - DTU::FIRST_FREE_EP;
}

EP EP::alloc() {
    return alloc_for(VPE::self());
}

EP EP::alloc_for(VPE &vpe) {
    if(USE_PEXCALLS) { // TODO actually: VPE.runs_on_pemux()
        epid_t id;
        capsel_t sel = alloc_cap(vpe, &id);
        return EP(sel, id, true);
    }

    epid_t id = vpe.epmng().alloc_ep();
    return EP(sel_of(vpe, id), id, true);
}

EP EP::bind(epid_t id) noexcept {
    return bind_for(VPE::self(), id);
}

EP EP::bind_for(VPE &vpe, epid_t id) noexcept {
    capsel_t sel = id == Gate::UNBOUND ? ObjCap::INVALID : sel_of(vpe, id);
    return EP(sel, id, false);
}

capsel_t EP::alloc_cap(VPE &vpe, epid_t *id) {
    capsel_t sel = VPE::self().alloc_sel();
    *id = VPE::self().resmng()->alloc_ep(sel, vpe.sel());
    return sel;
}

void EP::assign(Gate &gate) {
    DTUIf::switch_gate(*this, gate);
}

}
