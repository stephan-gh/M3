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

#include "Types.h"

namespace kernel {

class RGateObject;
class VPE;
class VPEDesc;

class TCU {
public:
    static const size_t SYSC_REP_COUNT = 2;

    static const epid_t SYSC_REPS = 0;
    static const epid_t SERV_REP = SYSC_REPS + SYSC_REP_COUNT;
    static const epid_t PEX_REP = SERV_REP + 1;
    static const epid_t TMP_MEP = PEX_REP + 1;
    static const epid_t TMP_SEP = TMP_MEP + 1;

    static void deprivilege(peid_t pe);

    static void init_vpe(peid_t pe);
    static void reset_pe(peid_t pe);

    static void config_recv(m3::TCU::reg_t *regs, vpeid_t vpe, goff_t buf, uint order,
                            uint msgorder, uint reply_eps);
    static void config_send(m3::TCU::reg_t *regs, vpeid_t vpe, label_t lbl, peid_t pe, epid_t dstep,
                            uint msgorder, uint credits);
    static void config_mem(m3::TCU::reg_t *regs, vpeid_t vpe, peid_t pe, vpeid_t tvpe, goff_t addr,
                           size_t size, uint perm);

    template<typename CFG>
    static void config_remote_ep(vpeid_t vpe, peid_t pe, epid_t ep, CFG config) {
        m3::TCU::reg_t ep_regs[m3::TCU::EP_REGS];
        config(ep_regs);
        write_ep_remote(vpe, pe, ep, ep_regs);
    }

    static m3::Errors::Code inv_reply_remote(peid_t pe, epid_t rep, peid_t rpe, epid_t sep);
    static m3::Errors::Code inval_ep_remote(vpeid_t vpe, peid_t pe, epid_t ep, bool force,
                                            uint32_t *unreadMask);

    static void write_ep_remote(vpeid_t vpe, peid_t pe, epid_t ep, const void *regs);
    static void write_ep_local(epid_t ep, const void *regs);
    static void update_eps(vpeid_t vpe, peid_t pe);

    static void recv_msgs(epid_t ep, uintptr_t buf, uint order, uint msgorder);

    static void reply(epid_t ep, const void *reply, size_t size, const m3::TCU::Message *msg);

    static void drop_msgs(epid_t rep, label_t label);
    static const m3::TCU::Message *fetch_msg(epid_t rep);
    static void ack_msg(epid_t rep, const m3::TCU::Message *msg);

    static m3::Errors::Code send_to(peid_t pe, epid_t ep, label_t label, const void *msg,
                                    size_t size, label_t replylbl, epid_t replyep);

    static m3::Errors::Code try_write_mem(const VPEDesc &vpe, goff_t addr,
                                          const void *data, size_t size);
    static m3::Errors::Code try_read_mem(const VPEDesc &vpe, goff_t addr, void *data, size_t size);

    static void write_mem(const VPEDesc &vpe, goff_t addr, const void *data, size_t size) {
        if(try_write_mem(vpe, addr, data, size) != m3::Errors::NONE)
            PANIC("write failed");
    }
    static void read_mem(const VPEDesc &vpe, goff_t addr, void *data, size_t size) {
        if(try_read_mem(vpe, addr, data, size) != m3::Errors::NONE)
            PANIC("read failed");
    }

    static void copy_clear(const VPEDesc &dstvpe, goff_t dstaddr,
                           const VPEDesc &srcvpe, goff_t srcaddr,
                           size_t size, bool clear);

private:
    template<typename CFG>
    static void config_local_ep(epid_t ep, CFG config) {
        m3::TCU::reg_t ep_regs[m3::TCU::EP_REGS];
        config(ep_regs);
        write_ep_local(ep, ep_regs);
    }

#if defined(__gem5__)
    static m3::Errors::Code do_ext_cmd(peid_t pe, m3::TCU::ExtCmdOpCode op, m3::TCU::reg_t *arg);
#endif
};

}
