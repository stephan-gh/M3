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

#pragma once

#include <base/Common.h>
#include <base/TCU.h>
#include <base/Panic.h>

#include "TCUState.h"

namespace kernel {

class RGateObject;
class VPE;
class VPEDesc;

class TCU {
    explicit TCU() : _state() {
    }

public:
    static const size_t SYSC_REP_COUNT = 2;

    static const epid_t SYSC_REPS = 0;
    static const epid_t SERV_REP = SYSC_REPS + SYSC_REP_COUNT;
    static const epid_t PEX_REP = SERV_REP + 1;
    static const epid_t TMP_MEP = PEX_REP + 1;
    static const epid_t TMP_SEP = TMP_MEP + 1;

    static TCU &get() {
        return _inst;
    }

    TCUState &state() {
        return _state;
    }

    void deprivilege(peid_t pe);

    void init_vpe(peid_t pe);
    void kill_vpe(peid_t pe);

    m3::Errors::Code inv_reply_remote(peid_t pe, epid_t rep, peid_t rpe, epid_t sep);

    m3::Errors::Code inval_ep_remote(peid_t pe, epid_t ep, bool force, uint32_t *unreadMask);
    void write_ep_remote(peid_t pe, epid_t ep, void *regs);
    void write_ep_local(epid_t ep);

    void recv_msgs(epid_t ep, uintptr_t buf, uint order, uint msgorder);

    void reply(epid_t ep, const void *reply, size_t size, const m3::TCU::Message *msg);

    m3::Errors::Code send_to(const VPEDesc &vpe, epid_t ep, label_t label, const void *msg,
                             size_t size, label_t replylbl, epid_t replyep);

    m3::Errors::Code try_write_mem(const VPEDesc &vpe, goff_t addr, const void *data, size_t size);
    m3::Errors::Code try_read_mem(const VPEDesc &vpe, goff_t addr, void *data, size_t size);

    void write_mem(const VPEDesc &vpe, goff_t addr, const void *data, size_t size) {
        if(try_write_mem(vpe, addr, data, size) != m3::Errors::NONE)
            PANIC("write failed");
    }
    void read_mem(const VPEDesc &vpe, goff_t addr, void *data, size_t size) {
        if(try_read_mem(vpe, addr, data, size) != m3::Errors::NONE)
            PANIC("read failed");
    }

    void copy_clear(const VPEDesc &dstvpe, goff_t dstaddr,
                    const VPEDesc &srcvpe, goff_t srcaddr,
                    size_t size, bool clear);

private:
#if defined(__gem5__)
    m3::Errors::Code do_ext_cmd(peid_t pe, m3::TCU::ExtCmdOpCode op, m3::TCU::reg_t *arg);
#endif

    TCUState _state;
    static TCU _inst;
};

}
