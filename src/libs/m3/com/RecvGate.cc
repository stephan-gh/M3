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

#include <base/log/Lib.h>
#include <base/Init.h>
#include <base/Panic.h>

#include <m3/com/RecvGate.h>
#include <m3/com/RecvBufs.h>
#include <m3/Exception.h>
#include <m3/Syscalls.h>
#include <m3/pes/VPE.h>

#include <thread/ThreadManager.h>

namespace m3 {

INIT_PRIO_RECVBUF RecvGate RecvGate::_syscall (
    KIF::INV_SEL,
    PEDesc(env()->pe_desc).rbuf_std_space().first,
    env()->first_std_ep + TCU::SYSC_REP_OFF,
    m3::nextlog2<SYSC_RBUF_SIZE>::val,
    SYSC_RBUF_ORDER,
    KEEP_CAP
);

INIT_PRIO_RECVBUF RecvGate RecvGate::_upcall (
    KIF::INV_SEL,
    PEDesc(env()->pe_desc).rbuf_std_space().first + SYSC_RBUF_SIZE,
    env()->first_std_ep + TCU::UPCALL_REP_OFF,
    m3::nextlog2<UPCALL_RBUF_SIZE>::val,
    UPCALL_RBUF_ORDER,
    KEEP_CAP
);

INIT_PRIO_RECVBUF RecvGate RecvGate::_default (
    KIF::INV_SEL,
    PEDesc(env()->pe_desc).rbuf_std_space().first + SYSC_RBUF_SIZE + UPCALL_RBUF_SIZE,
    env()->first_std_ep + TCU::DEF_REP_OFF,
    m3::nextlog2<DEF_RBUF_SIZE>::val,
    DEF_RBUF_ORDER,
    KEEP_CAP
);

void RecvGate::RecvGateWorkItem::work() {
    const TCU::Message *msg = TCUIf::fetch_msg(*_gate);
    if(msg) {
        LLOG(IPC, "Received msg @ " << (void*)msg << " over ep " << _gate->ep());
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
    return RecvGate(VPE::self().alloc_sel(), 0, UNBOUND, order, msgorder, 0);
}

RecvGate RecvGate::create(capsel_t cap, uint order, uint msgorder, uint flags) {
    return RecvGate(cap, 0, UNBOUND, order, msgorder, flags);
}

RecvGate RecvGate::bind(capsel_t cap, uint order, uint msgorder) noexcept {
    return RecvGate(cap, 0, order, msgorder, KEEP_CAP);
}

RecvGate::~RecvGate() {
    deactivate();
    if(_buf)
        RecvBufs::get().free(_buf);
}

uintptr_t RecvGate::address() const noexcept {
    return _buf_addr;
}

void RecvGate::activate() {
    if(!this->ep()) {
        if(_buf == nullptr) {
            _buf = RecvBufs::get().alloc(1UL << _order);
            _buf_addr = _buf->addr();
        }

        auto rep = VPE::self().epmng().acquire(EP_COUNT, slots());
        Gate::activate_on(*rep, _buf->addr());
        Gate::set_ep(rep);
    }
}

void RecvGate::activate_on(const EP &ep, uintptr_t addr) {
    Gate::activate_on(ep, addr);
}

void RecvGate::deactivate() noexcept {
    release_ep(VPE::self(), true);

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

const TCU::Message *RecvGate::fetch() {
    activate();
    return TCUIf::fetch_msg(*this);
}

void RecvGate::reply(const void *reply, size_t len, const TCU::Message *msg) {
    Errors::Code res = TCUIf::reply(*this, reply, len, msg);
    if(EXPECT_FALSE(res != Errors::NONE))
        throw TCUException(res);
}

const TCU::Message *RecvGate::receive(SendGate *sgate) {
    activate();
    const TCU::Message *reply = nullptr;
    Errors::Code res = TCUIf::receive(*this, sgate, &reply);
    if(res != Errors::NONE)
        throw MessageException("SendGate became invalid while waiting for reply", res);
    return reply;
}

void RecvGate::ack_msg(const TCU::Message *msg) {
    TCUIf::ack_msg(*this, msg);
}

void RecvGate::drop_msgs_with(label_t label) noexcept {
    TCUIf::drop_msgs(*this, label);
}

}
