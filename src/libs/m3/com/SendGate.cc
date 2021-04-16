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

#include <m3/com/SendGate.h>
#include <m3/Exception.h>
#include <m3/Syscalls.h>
#include <m3/pes/VPE.h>

#include <thread/ThreadManager.h>

#include <assert.h>

namespace m3 {

SendGate SendGate::create(RecvGate *rgate, const SendGateArgs &args) {
    auto replygate = args._replygate == nullptr ? &RecvGate::def() : args._replygate;
    auto sel = args._sel == INVALID ? VPE::self().alloc_sel() : args._sel;
    Syscalls::create_sgate(sel, rgate->sel(), args._label, args._credits);
    return SendGate(sel, args._flags, replygate);
}

uint SendGate::credits() {
    const EP &sep = activate();
    if(!TCU::get().is_valid(sep.id()))
        throw Exception(Errors::NO_SEP);
    return TCU::get().credits(sep.id());
}

void SendGate::send(const MsgBuf &msg, label_t reply_label) {
    Errors::Code res = try_send(msg, reply_label);
    if(res != Errors::NONE)
        throw TCUException(res);
}

void SendGate::send_aligned(const void *msg, size_t len, label_t reply_label) {
    Errors::Code res = try_send_aligned(msg, len, reply_label);
    if(res != Errors::NONE)
        throw TCUException(res);
}

Errors::Code SendGate::try_send(const MsgBuf &msg, label_t reply_label) {
    return try_send_aligned(msg.bytes(), msg.size(), reply_label);
}

Errors::Code SendGate::try_send_aligned(const void *msg, size_t len, label_t reply_label) {
    const EP &sep = activate();
    epid_t rep = _replygate->ep() ? _replygate->ep()->id() : TCU::NO_REPLIES;
    return TCU::get().send_aligned(sep.id(), msg, len, reply_label, rep);
}

const TCU::Message *SendGate::call(const MsgBuf &msg) {
    send(msg, 0);
    return _replygate->receive(this);
}

}
