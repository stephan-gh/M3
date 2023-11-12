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

#include <base/Init.h>
#include <base/Log.h>
#include <base/Panic.h>

#include <m3/Exception.h>
#include <m3/Syscalls.h>
#include <m3/com/RecvBufs.h>
#include <m3/com/RecvGate.h>
#include <m3/session/ResMng.h>
#include <m3/tiles/Activity.h>

#include <thread/ThreadManager.h>

namespace m3 {

RecvCap::RecvCap(capsel_t sel, uint order, uint msgorder, uint flags, bool create)
    : ObjCap(RECV_GATE, sel, flags),
      _order(order),
      _msgorder(msgorder) {
    if(create)
        Syscalls::create_rgate(sel, order, msgorder);
}

RecvCap RecvCap::create(uint order, uint msgorder) {
    return RecvCap(SelSpace::get().alloc_sel(), order, msgorder, 0, true);
}

RecvCap RecvCap::create(capsel_t cap, uint order, uint msgorder) {
    return RecvCap(cap, order, msgorder, 0, true);
}

RecvCap RecvCap::create_named(const char *name) {
    auto sel = SelSpace::get().alloc_sel();
    auto args = Activity::own().resmng()->use_rgate(sel, name);
    return RecvCap(sel, args.first, args.second, 0, false);
}

RecvCap RecvCap::bind(capsel_t cap) noexcept {
    return RecvCap(cap, 0, 0, KEEP_CAP, false);
}

void RecvCap::fetch_buffer_size() const {
    if(_order == 0) {
        auto size = Syscalls::rgate_buffer(sel());
        _order = size.first;
        _msgorder = size.second;
    }
}

RecvGate RecvCap::activate() {
    fetch_buffer_size();
    auto buf = RecvBufs::get().alloc(1UL << _order);
    size_t buf_addr = buf->addr();

    auto rep = EPMng::get().acquire(TOTAL_EPS, slots());
    Gate::activate_on(sel(), *rep, buf->mem(), buf->off());

    // prevent that we revoke the cap
    auto cap_flags = flags();
    flags(KEEP_CAP);

    return RecvGate(sel(), buf_addr, buf, rep, _order, _msgorder, cap_flags);
}

void RecvCap::activate_on(const EP &ep, MemGate *mem, size_t off) {
    Gate::activate_on(sel(), ep, mem ? mem->sel() : KIF::INV_SEL, off);
}

INIT_PRIO_RECVGATE RecvGate
    RecvGate::_syscall(KIF::INV_SEL, TileDesc(env()->tile_desc).rbuf_std_space().first, nullptr,
                       new EP(EP::bind(env()->first_std_ep + TCU::SYSC_REP_OFF)),
                       m3::nextlog2<SYSC_RBUF_SIZE>::val, SYSC_RBUF_ORDER, KEEP_CAP);

INIT_PRIO_RECVGATE RecvGate RecvGate::_upcall(
    KIF::INV_SEL, TileDesc(env()->tile_desc).rbuf_std_space().first + SYSC_RBUF_SIZE, nullptr,
    new EP(EP::bind(env()->first_std_ep + TCU::UPCALL_REP_OFF)),
    m3::nextlog2<UPCALL_RBUF_SIZE>::val, UPCALL_RBUF_ORDER, KEEP_CAP);

INIT_PRIO_RECVGATE RecvGate RecvGate::_default(
    KIF::INV_SEL,
    TileDesc(env()->tile_desc).rbuf_std_space().first + SYSC_RBUF_SIZE + UPCALL_RBUF_SIZE, nullptr,
    new EP(EP::bind(env()->first_std_ep + TCU::DEF_REP_OFF)), m3::nextlog2<DEF_RBUF_SIZE>::val,
    DEF_RBUF_ORDER, KEEP_CAP);

void RecvGate::RecvGateWorkItem::work() {
    const TCU::Message *msg = _gate->fetch();
    if(msg) {
        GateIStream is(*_gate, msg);
        _gate->_handler(is);
    }
}

RecvGate::RecvGate(capsel_t cap, size_t addr, RecvBuf *buf, EP *ep, uint order, uint msgorder,
                   uint flags) noexcept
    : Gate(RECV_GATE, cap, flags),
      _buf(buf),
      _buf_addr(addr),
      _order(order),
      _msgorder(msgorder),
      _handler(),
      _workitem() {
    set_ep(ep);
}

RecvGate::~RecvGate() {
    release_ep(true);
    stop();
    if(_buf)
        RecvBufs::get().free(_buf);
}

void RecvGate::start(WorkLoop *wl, msghandler_t handler) {
    assert(!_workitem);
    _handler = handler;

    _workitem = std::make_unique<RecvGateWorkItem>(this);
    wl->add(_workitem.get(), ep()->is_standard());
}

void RecvGate::stop() noexcept {
    _workitem.reset();
}

void RecvGate::wait_for_msg() {
    OwnActivity::wait_for_msg(ep()->id());
}

const TCU::Message *RecvGate::fetch() noexcept {
    size_t msg_off = TCU::get().fetch_msg(ep()->id());
    if(msg_off != static_cast<size_t>(-1))
        return TCU::offset_to_msg(address(), msg_off);
    return nullptr;
}

bool RecvGate::has_msgs() noexcept {
    return TCU::get().has_msgs(ep()->id());
}

void RecvGate::reply_aligned(const void *reply, size_t len, const TCU::Message *msg) {
    size_t msg_off = TCU::msg_to_offset(address(), msg);
    Errors::Code res = TCU::get().reply_aligned(ep()->id(), reply, len, msg_off);
    if(EXPECT_FALSE(res != Errors::SUCCESS))
        throw TCUException(res);
}

const TCU::Message *RecvGate::receive(SendGate *sgate) {
    // if the tile is shared with someone else that wants to run, poll a couple of times to
    // prevent too frequent/unnecessary switches.
    int polling = env()->shared ? 200 : 1;
    while(1) {
        for(int i = 0; i < polling; ++i) {
            const TCU::Message *reply = fetch();
            if(reply)
                return reply;
        }

        if(sgate && EXPECT_FALSE(!TCU::get().is_valid(sgate->ep()->id()))) {
            throw MessageException("SendGate became invalid while waiting for reply",
                                   Errors::EP_INVALID);
        }

        OwnActivity::wait_for_msg(ep()->id());
    }
    UNREACHED;
}

void RecvGate::ack_msg(const TCU::Message *msg) noexcept {
    size_t msg_off = TCU::msg_to_offset(address(), msg);
    TCU::get().ack_msg(ep()->id(), msg_off);
}

void RecvGate::drop_msgs_with(label_t label) noexcept {
    TCU::get().drop_msgs(address(), ep()->id(), label);
}

}
