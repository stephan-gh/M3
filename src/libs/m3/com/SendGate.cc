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

#include <m3/Exception.h>
#include <m3/Syscalls.h>
#include <m3/com/SendGate.h>
#include <m3/session/ResMng.h>
#include <m3/tiles/Activity.h>

#include <assert.h>
#include <thread/ThreadManager.h>

namespace m3 {

SendCap SendCap::create(RecvGate *rgate, const SendGateArgs &args) {
    auto sel = args._sel == INVALID ? Activity::own().alloc_sel() : args._sel;
    Syscalls::create_sgate(sel, rgate->sel(), args._label, args._credits);
    return SendCap(sel, args._flags, args._reply_gate);
}

SendCap SendCap::create_named(const char *name, RecvGate *reply_gate) {
    auto sel = Activity::own().alloc_sel();
    Activity::own().resmng()->use_sgate(sel, name);
    return SendCap(sel, 0, reply_gate);
}

SendGate SendCap::activate() {
    auto org_flags = flags();

    EP *ep = Activity::own().epmng().acquire();
    activate_on(*ep);

    // don't revoke the cap
    flags(KEEP_CAP);

    return SendGate(sel(), org_flags, _reply_gate, ep);
}

void SendCap::activate_on(const EP &ep) {
    Syscalls::activate(ep.sel(), sel(), KIF::INV_SEL, 0);
}

uint SendGate::credits() {
    const EP *sep = ep();
    if(!TCU::get().is_valid(sep->id()))
        throw Exception(Errors::NO_SEP);
    return TCU::get().credits(sep->id());
}

void SendGate::send(const MsgBuf &msg, label_t reply_label) {
    Errors::Code res = try_send(msg, reply_label);
    if(res != Errors::SUCCESS)
        throw TCUException(res);
}

void SendGate::send_aligned(const void *msg, size_t len, label_t reply_label) {
    Errors::Code res = try_send_aligned(msg, len, reply_label);
    if(res != Errors::SUCCESS)
        throw TCUException(res);
}

Errors::Code SendGate::try_send(const MsgBuf &msg, label_t reply_label) {
    return try_send_aligned(msg.bytes(), msg.size(), reply_label);
}

Errors::Code SendGate::try_send_aligned(const void *msg, size_t len, label_t reply_label) {
    const EP *sep = ep();
    epid_t rep = _reply_gate->ep() ? _reply_gate->ep()->id() : TCU::NO_REPLIES;
    return TCU::get().send_aligned(sep->id(), msg, len, reply_label, rep);
}

const TCU::Message *SendGate::call(const MsgBuf &msg) {
    send(msg, 0);
    return _reply_gate->receive(this);
}

}
