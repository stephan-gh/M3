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

INIT_PRIO_RECVGATE RecvGate RecvGate::_syscall(KIF::INV_SEL,
                                               TileDesc(env()->tile_desc).rbuf_std_space().first,
                                               env()->first_std_ep + TCU::SYSC_REP_OFF,
                                               m3::nextlog2<SYSC_RBUF_SIZE>::val, SYSC_RBUF_ORDER,
                                               KEEP_CAP);

INIT_PRIO_RECVGATE RecvGate RecvGate::_upcall(KIF::INV_SEL,
                                              TileDesc(env()->tile_desc).rbuf_std_space().first +
                                                  SYSC_RBUF_SIZE,
                                              env()->first_std_ep + TCU::UPCALL_REP_OFF,
                                              m3::nextlog2<UPCALL_RBUF_SIZE>::val,
                                              UPCALL_RBUF_ORDER, KEEP_CAP);

INIT_PRIO_RECVGATE RecvGate RecvGate::_default(KIF::INV_SEL,
                                               TileDesc(env()->tile_desc).rbuf_std_space().first +
                                                   SYSC_RBUF_SIZE + UPCALL_RBUF_SIZE,
                                               env()->first_std_ep + TCU::DEF_REP_OFF,
                                               m3::nextlog2<DEF_RBUF_SIZE>::val, DEF_RBUF_ORDER,
                                               KEEP_CAP);

void RecvGate::RecvGateWorkItem::work() {
    const TCU::Message *msg = _gate->fetch();
    if(msg) {
        GateIStream is(*_gate, msg);
        _gate->_handler(is);
    }
}

RecvGate::RecvGate(capsel_t cap, size_t addr, epid_t ep, uint order, uint msgorder, uint flags)
    : Gate(RECV_GATE, cap, flags),
      _buf(),
      _buf_addr(addr),
      _order(order),
      _msgorder(msgorder),
      _handler(),
      _workitem() {
    if(sel() != ObjCap::INVALID && sel() >= KIF::FIRST_FREE_SEL)
        Syscalls::create_rgate(sel(), order, msgorder);

    if(ep != UNBOUND)
        set_ep(ep);
}

RecvGate RecvGate::create(uint order, uint msgorder) {
    return RecvGate(SelSpace::get().alloc_sel(), 0, UNBOUND, order, msgorder, 0);
}

RecvGate RecvGate::create(capsel_t cap, uint order, uint msgorder, uint flags) {
    return RecvGate(cap, 0, UNBOUND, order, msgorder, flags);
}

RecvGate RecvGate::create_named(const char *name) {
    auto sel = SelSpace::get().alloc_sel();
    auto args = Activity::own().resmng()->use_rgate(sel, name);
    return RecvGate(sel, 0, args.first, args.second, 0);
}

RecvGate RecvGate::bind(capsel_t cap) noexcept {
    return RecvGate(cap, 0, 0, 0, KEEP_CAP);
}

RecvGate::~RecvGate() {
    deactivate();
    if(_buf)
        RecvBufs::get().free(_buf);
}

uintptr_t RecvGate::address() const noexcept {
    return _buf_addr;
}

void RecvGate::fetch_buffer_size() const {
    if(_order == 0) {
        auto size = Syscalls::rgate_buffer(sel());
        _order = size.first;
        _msgorder = size.second;
    }
}

void RecvGate::activate() {
    if(!this->ep()) {
        fetch_buffer_size();
        if(_buf == nullptr) {
            _buf = RecvBufs::get().alloc(1UL << _order);
            _buf_addr = _buf->addr();
        }

        auto rep = EPMng::get().acquire(TOTAL_EPS, slots());
        Gate::activate_on(*rep, _buf->mem(), _buf->off());
        Gate::set_ep(rep);
    }
}

void RecvGate::activate_on(const EP &ep, MemGate *mem, size_t off) {
    Gate::activate_on(ep, mem ? mem->sel() : KIF::INV_SEL, off);
}

void RecvGate::deactivate() noexcept {
    release_ep(true);

    stop();
}

void RecvGate::start(WorkLoop *wl, msghandler_t handler) {
    activate();

    assert(!_workitem);
    _handler = handler;

    _workitem = std::make_unique<RecvGateWorkItem>(this);
    wl->add(_workitem.get(), ep()->is_standard());
}

void RecvGate::stop() noexcept {
    _workitem.reset();
}

void RecvGate::wait_for_msg() {
    activate();
    OwnActivity::wait_for_msg(ep()->id());
}

const TCU::Message *RecvGate::fetch() {
    activate();
    size_t msg_off = TCU::get().fetch_msg(ep()->id());
    if(msg_off != static_cast<size_t>(-1))
        return TCU::offset_to_msg(address(), msg_off);
    return nullptr;
}

bool RecvGate::has_msgs() {
    activate();
    return TCU::get().has_msgs(ep()->id());
}

void RecvGate::reply_aligned(const void *reply, size_t len, const TCU::Message *msg) {
    size_t msg_off = TCU::msg_to_offset(address(), msg);
    Errors::Code res = TCU::get().reply_aligned(ep()->id(), reply, len, msg_off);
    if(EXPECT_FALSE(res != Errors::SUCCESS))
        throw TCUException(res);
}

const TCU::Message *RecvGate::receive(SendGate *sgate) {
    activate();

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

void RecvGate::ack_msg(const TCU::Message *msg) {
    size_t msg_off = TCU::msg_to_offset(address(), msg);
    TCU::get().ack_msg(ep()->id(), msg_off);
}

void RecvGate::drop_msgs_with(label_t label) noexcept {
    TCU::get().drop_msgs(address(), ep()->id(), label);
}

}
