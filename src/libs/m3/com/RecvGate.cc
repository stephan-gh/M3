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
#if defined(__host__) || defined(__gem5__)
    VPE::self(), ObjCap::INVALID, DTU::SYSC_REP, get_rgate_buf(0),
        m3::nextlog2<SYSC_RBUF_SIZE>::val, SYSC_RBUF_ORDER, 0
#else
    VPE::self(), ObjCap::INVALID, DTU::SYSC_REP, reinterpret_cast<void*>(DEF_RCVBUF),
        DEF_RCVBUF_MSGORDER, DEF_RCVBUF_MSGORDER, 0
#endif
);

INIT_PRIO_RECVBUF RecvGate RecvGate::_upcall (
    VPE::self(), ObjCap::INVALID, DTU::UPCALL_REP, get_rgate_buf(SYSC_RBUF_SIZE),
        m3::nextlog2<UPCALL_RBUF_SIZE>::val, UPCALL_RBUF_ORDER, 0
);

INIT_PRIO_RECVBUF RecvGate RecvGate::_default (
    VPE::self(), ObjCap::INVALID, DTU::DEF_REP, get_rgate_buf(SYSC_RBUF_SIZE + UPCALL_RBUF_SIZE),
        m3::nextlog2<DEF_RBUF_SIZE>::val, DEF_RBUF_ORDER, 0
);


INIT_PRIO_RECVBUF RecvGate RecvGate::_invalid (
    VPE::self(), ObjCap::INVALID, UNBOUND, nullptr, 0, 0, 0
);

void RecvGate::RecvGateWorkItem::work() {
    DTU::Message *msg = DTU::get().fetch_msg(_buf->ep());
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
    if(sel() != ObjCap::INVALID)
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

RecvGate RecvGate::bind(capsel_t cap, int order, epid_t ep) noexcept {
    RecvGate rgate(VPE::self(), cap, order, KEEP_CAP);
    if(ep != EP_COUNT)
        rgate.ep(ep);
    return rgate;
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

#if defined(__t3__)
    // required for t3 because one can't write to these registers externally
    DTU::get().configure_recv(ep(), addr, order(), msgorder(), flags());
#endif

    if(sel() != ObjCap::INVALID)
        Syscalls::activate(_vpe.ep_to_sel(ep()), sel(), addr);
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
    _workitem = std::unique_ptr<RecvGateWorkItem>(new RecvGateWorkItem(this));
    wl->add(_workitem.get(), permanent);
}

void RecvGate::stop() noexcept {
    _workitem.reset();
}

void RecvGate::reply(const void *data, size_t len, size_t msgidx) {
    Errors::Code res = DTU::get().reply(ep(), const_cast<void*>(data), len, msgidx);

    if(EXPECT_FALSE(res == Errors::VPE_GONE)) {
        event_t event = ThreadManager::get().get_wait_event();
        bool upcall = Syscalls::forward_reply(sel(), data, len, msgidx, event);

        // if this has been done, go to sleep and wait until the kernel sends us the upcall
        if(upcall) {
            ThreadManager::get().wait_for(event);
            auto *msg = reinterpret_cast<const KIF::Upcall::Forward*>(
                ThreadManager::get().get_current_msg());
            res = static_cast<Errors::Code>(msg->error);
            if(res != Errors::NONE)
                throw SyscallException(res, KIF::Syscall::FORWARD_REPLY);
        }
    }
    else if(EXPECT_FALSE(res != Errors::NONE))
        throw DTUException(res);
}

const DTU::Message *RecvGate::wait(SendGate *sgate) {
    activate();

    while(1) {
        const DTU::Message *msg = DTU::get().fetch_msg(ep());
        if(msg)
            return msg;

        // fetch the events first
        DTU::get().fetch_events();
        // now check whether the endpoint is still valid. if the EP has been invalidated before the
        // line above, we'll notice that with this check. if the EP is invalidated between the line
        // above and the sleep command, the DTU will refuse to suspend the core.
        if(sgate && !DTU::get().is_valid(sgate->ep()))
            throw MessageException("SendGate became invalid while waiting for reply", Errors::EP_INVALID);

        DTU::get().try_sleep(true);
    }
    UNREACHED;
}

}
