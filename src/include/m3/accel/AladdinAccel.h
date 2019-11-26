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

#pragma once

#include <m3/com/GateStream.h>
#include <m3/com/RecvGate.h>
#include <m3/com/SendGate.h>
#include <m3/session/Pager.h>
#include <m3/pes/VPE.h>

#include <memory>

namespace m3 {

class AladdinAccel {
public:
    static const uint DATA_EP       = 16;
    static const uint RECV_EP       = 17;
    static const size_t RB_SIZE     = 256;

    static const size_t BUF_SIZE    = 1024;
    static const size_t BUF_ADDR    = 0x8000;
    static const size_t STATE_SIZE  = 1024;
    static const size_t STATE_ADDR  = BUF_ADDR - STATE_SIZE;

    struct Array {
        uint64_t addr;
        uint64_t size;
    } PACKED;

    struct InvokeMessage {
        Array arrays[8];
        uint64_t array_count;
        uint64_t iterations;
        uint64_t repeats;
    } PACKED;

    explicit AladdinAccel(PEISA isa, const char *name, const char *pager)
        : _pe(PE::alloc(PEDesc(PEType::COMP_EMEM, isa))),
          _accel(_pe, name, VPEArgs().pager(pager)),
          _lastmem(ObjCap::INVALID),
          _rgate(RecvGate::create(nextlog2<256>::val, nextlog2<256>::val)),
          _srgate(RecvGate::create_for(_accel, getnextlog2(RB_SIZE), getnextlog2(RB_SIZE))),
          _sgate(SendGate::create(&_srgate, SendGateArgs().credits(1).reply_gate(&_rgate))),
          _rep(_accel.epmng().acquire(RECV_EP, _srgate.slots())) {
        // has to be activated
        _rgate.activate();

        if(_accel.pager()) {
            goff_t virt = STATE_ADDR;
            _accel.pager()->map_anon(&virt, STATE_SIZE + BUF_SIZE, Pager::Prot::RW, 0);
        }

        _srgate.activate_on(*_rep);
        _accel.start();
    }

    VPE &vpe() noexcept {
        return _accel;
    }
    PEISA isa() const noexcept {
        return _accel.pe_desc().isa();
    }

    void start(const InvokeMessage &msg) {
        send_msg(_sgate, &msg, sizeof(msg));
    }
    uint64_t wait() {
        GateIStream is = receive_reply(_sgate);
        uint64_t res;
        is >> res;
        return res;
    }
    uint64_t invoke(const InvokeMessage &msg) {
        start(msg);
        return wait();
    }

private:
    Reference<PE> _pe;
    VPE _accel;
    capsel_t _lastmem;
    m3::RecvGate _rgate;
    m3::RecvGate _srgate;
    m3::SendGate _sgate;
    std::unique_ptr<EP> _rep;
};

}
