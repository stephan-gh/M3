/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
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

#include <m3/Syscalls.h>
#include <m3/session/ClientSession.h>
#include <m3/session/ResMng.h>
#include <m3/tiles/Activity.h>

namespace m3 {

ClientSession::~ClientSession() {
    if(_close && sel() != INVALID) {
        try {
            Activity::own().resmng()->close_sess(sel());
        }
        catch(...) {
            // ignore
        }
        flags(0);
    }
}

void ClientSession::open(const std::string_view &service, capsel_t selector) {
    if(selector == INVALID)
        selector = SelSpace::get().alloc_sel();

    Activity::own().resmng()->open_sess(selector, service);
    sel(selector);
}

SendGate ClientSession::connect() {
    auto sel = SelSpace::get().alloc_sel();
    return SendGate::bind(connect_for(Activity::own(), sel));
}

capsel_t ClientSession::connect_for(Activity &act, capsel_t sel) {
    KIF::ExchangeArgs args;
    ExchangeOStream os(args);
    os << opcodes::General::CONNECT;
    args.bytes = os.total();
    obtain_for(act, KIF::CapRngDesc(KIF::CapRngDesc::OBJ, sel, 1), &args);
    return sel;
}

void ClientSession::delegate(const KIF::CapRngDesc &caps, KIF::ExchangeArgs *args) {
    delegate_for(Activity::own(), caps, args);
}

void ClientSession::delegate_for(Activity &act, const KIF::CapRngDesc &crd,
                                 KIF::ExchangeArgs *args) {
    Syscalls::delegate(act.sel(), sel(), crd, args);
}

KIF::CapRngDesc ClientSession::obtain(uint count, KIF::ExchangeArgs *args) {
    return obtain_for(Activity::own(), count, args);
}

KIF::CapRngDesc ClientSession::obtain_for(Activity &act, uint count, KIF::ExchangeArgs *args) {
    KIF::CapRngDesc crd(KIF::CapRngDesc::OBJ, SelSpace::get().alloc_sels(count), count);
    obtain_for(act, crd, args);
    return crd;
}

void ClientSession::obtain_for(Activity &act, const KIF::CapRngDesc &crd, KIF::ExchangeArgs *args) {
    Syscalls::obtain(act.sel(), sel(), crd, args);
}

}
