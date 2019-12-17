/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <m3/session/ClientSession.h>
#include <m3/session/ResMng.h>
#include <m3/Syscalls.h>
#include <m3/pes/VPE.h>

namespace m3 {

ClientSession::~ClientSession() {
    if(_close && sel() != INVALID) {
        try {
            VPE::self().resmng()->close_sess(sel());
        }
        catch(...) {
            // ignore
        }
        flags(0);
    }
}

void ClientSession::connect(const String &service, capsel_t selector) {
    if(selector == INVALID)
        selector = VPE::self().alloc_sel();

    VPE::self().resmng()->open_sess(selector, service);
    sel(selector);
}

void ClientSession::delegate(const KIF::CapRngDesc &caps, KIF::ExchangeArgs *args) {
    delegate_for(VPE::self(), caps, args);
}

void ClientSession::delegate_for(VPE &vpe, const KIF::CapRngDesc &crd, KIF::ExchangeArgs *args) {
    Syscalls::delegate(vpe.sel(), sel(), crd, args);
}

KIF::CapRngDesc ClientSession::obtain(uint count, KIF::ExchangeArgs *args) {
    return obtain_for(VPE::self(), count, args);
}

KIF::CapRngDesc ClientSession::obtain_for(VPE &vpe, uint count, KIF::ExchangeArgs *args) {
    KIF::CapRngDesc crd(KIF::CapRngDesc::OBJ, vpe.alloc_sels(count), count);
    obtain_for(vpe, crd, args);
    return crd;
}

void ClientSession::obtain_for(VPE &vpe, const KIF::CapRngDesc &crd, KIF::ExchangeArgs *args) {
    vpe.mark_caps_allocated(crd.start(), crd.count());
    Syscalls::obtain(vpe.sel(), sel(), crd, args);
}

}
