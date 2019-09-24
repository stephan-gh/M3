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

#include <m3/com/EPMux.h>
#include <m3/com/RecvGate.h>
#include <m3/Exception.h>
#include <m3/Syscalls.h>
#include <m3/VPE.h>

#include <thread/ThreadManager.h>

namespace m3 {

static void *get_rgate_buf(UNUSED size_t off) {
#if defined(__gem5__)
    PEDesc desc(env()->pe);
    if(desc.has_virtmem())
        return reinterpret_cast<void*>(RECVBUF_SPACE + off);
    else
        return reinterpret_cast<void*>((desc.mem_size() - RECVBUF_SIZE_SPM) + off);
#else
    return reinterpret_cast<void*>(Env::rbuf_start() + off);
#endif
}

INIT_PRIO_RECVBUF RecvGate RecvGate::_syscall (
    VPE::self(), KIF::SEL_SYSC_RG, DTU::SYSC_REP, get_rgate_buf(0),
        m3::nextlog2<SYSC_RBUF_SIZE>::val, SYSC_RBUF_ORDER, KEEP_CAP
);

INIT_PRIO_RECVBUF RecvGate RecvGate::_upcall (
    VPE::self(), KIF::SEL_UPC_RG, DTU::UPCALL_REP, get_rgate_buf(SYSC_RBUF_SIZE),
        m3::nextlog2<UPCALL_RBUF_SIZE>::val, UPCALL_RBUF_ORDER, KEEP_CAP
);

INIT_PRIO_RECVBUF RecvGate RecvGate::_default (
    VPE::self(), KIF::SEL_DEF_RG, DTU::DEF_REP, get_rgate_buf(SYSC_RBUF_SIZE + UPCALL_RBUF_SIZE),
        m3::nextlog2<DEF_RBUF_SIZE>::val, DEF_RBUF_ORDER, KEEP_CAP
);

INIT_PRIO_RECVBUF RecvGate RecvGate::_invalid (
    VPE::self(), ObjCap::INVALID, UNBOUND, nullptr, 0, 0, 0
);

void RecvGate::RecvGateWorkItem::work() {
    const DTU::Message *msg = DTUIf::fetch_msg(*_buf);
    if(msg) {
        LLOG(IPC, "Received msg @ " << (void*)msg << " over ep " << _buf->ep());
        GateIStream is(*_buf, msg);
        _buf->_handler(is);
    }
}

RecvGate::RecvGate(VPE &vpe, capsel_t cap, epid_t ep, void *buf, int order, int msgorder, uint flags)
    : Gate(RECV_GATE, cap, flags),
      _vpe(vpe),
      _buf(buf),
      _order(order),
      _free(0),
      _handler(),
      _workitem() {
    if(sel() != ObjCap::INVALID && sel() >= KIF::FIRST_FREE_SEL)
        Syscalls::create_rgate(sel(), order, msgorder);

    if(ep != UNBOUND)
        activate(ep);
}

RecvGate RecvGate::create(int order, int msgorder) {
    return create_for(VPE::self(), order, msgorder);
}

RecvGate RecvGate::create(capsel_t cap, int order, int msgorder) {
    return create_for(VPE::self(), cap, order, msgorder);
}

RecvGate RecvGate::create_for(VPE &vpe, int order, int msgorder) {
    return RecvGate(vpe, VPE::self().alloc_sel(), UNBOUND, nullptr, order, msgorder, 0);
}

RecvGate RecvGate::create_for(VPE &vpe, capsel_t cap, int order, int msgorder, uint flags) {
    return RecvGate(vpe, cap, UNBOUND, nullptr, order, msgorder, flags);
}

RecvGate RecvGate::bind(capsel_t cap, int order) noexcept {
    return RecvGate(VPE::self(), cap, order, KEEP_CAP);
}

RecvGate::~RecvGate() {
    if(_free & FREE_BUF)
        free(_buf);
    deactivate();
}

void RecvGate::activate() {
    if(ep() == UNBOUND) {
        epid_t ep = _vpe.alloc_ep();
        _free |= FREE_EP;
        activate(ep);
    }
}

void RecvGate::activate(epid_t _ep) {
    if(ep() == UNBOUND) {
        if(_buf == nullptr) {
            _buf = allocate(_vpe, _ep, 1UL << _order);
            _free |= FREE_BUF;
        }

        activate(_ep, reinterpret_cast<uintptr_t>(_buf));
    }
}

void RecvGate::activate(epid_t _ep, uintptr_t addr) {
    assert(ep() == UNBOUND);

    ep(_ep);

    if(sel() != ObjCap::INVALID && sel() >= KIF::FIRST_FREE_SEL) {
        if(&_vpe == &VPE::self())
            DTUIf::activate_gate(*this, ep(), addr);
        else
            Syscalls::activate(_vpe.ep_to_sel(ep()), sel(), addr);
    }
}

void RecvGate::deactivate() noexcept {
    if(_free & FREE_EP) {
        _vpe.free_ep(ep());
        _free &= ~static_cast<uint>(FREE_EP);
    }
    ep(UNBOUND);

    stop();
}

void RecvGate::start(WorkLoop *wl, msghandler_t handler) {
    activate();

    assert(&_vpe == &VPE::self());
    assert(!_workitem);
    _handler = handler;

    bool permanent = ep() < DTU::FIRST_FREE_EP;
    _workitem = std::make_unique<RecvGateWorkItem>(this);
    wl->add(_workitem.get(), permanent);
}

void RecvGate::stop() noexcept {
    _workitem.reset();
}

const DTU::Message *RecvGate::fetch() {
    activate();
    return DTUIf::fetch_msg(*this);
}

void RecvGate::reply(const void *reply, size_t len, const DTU::Message *msg) {
    Errors::Code res = DTUIf::reply(*this, reply, len, msg);
    if(EXPECT_FALSE(res != Errors::NONE))
        throw DTUException(res);
}

const DTU::Message *RecvGate::receive(SendGate *sgate) {
    activate();
    const DTU::Message *reply = nullptr;
    Errors::Code res = DTUIf::receive(*this, sgate, &reply);
    if(res != Errors::NONE)
        throw MessageException("SendGate became invalid while waiting for reply", res);
    return reply;
}

void RecvGate::mark_read(const DTU::Message *msg) {
    DTUIf::mark_read(*this, msg);
}

void RecvGate::drop_msgs_with(label_t label) noexcept {
    DTUIf::drop_msgs(ep(), label);
}

}
