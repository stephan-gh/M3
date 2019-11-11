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
#include <base/DTU.h>
#include <base/Panic.h>

#include "DTUState.h"
#include "SyscallHandler.h"

namespace kernel {

class RGateObject;
class VPE;
class VPEDesc;

class DTU {
    explicit DTU() : _ep(SyscallHandler::memep()) {
    }

public:
    static DTU &get() {
        return _inst;
    }

    DTUState &state() {
        return _state;
    }

    gaddr_t deprivilege(peid_t pe);

    void start_vpe(const VPEDesc &vpe);
    void kill_vpe(const VPEDesc &vpe, gaddr_t idle_rootpt);

    cycles_t get_time();
    void wakeup(const VPEDesc &vpe);
    void suspend(const VPEDesc &vpe);
    void inject_irq(const VPEDesc &vpe);
    void ext_request(const VPEDesc &vpe, uint64_t req);
    void flush_cache(const VPEDesc &vpe);

    void invtlb_remote(const VPEDesc &vpe);
    void invlpg_remote(const VPEDesc &vpe, goff_t virt);

    m3::Errors::Code inv_reply_remote(const VPEDesc &vpe, epid_t rep, peid_t pe, epid_t sep);

    m3::Errors::Code inval_ep_remote(const VPEDesc &vpe, epid_t ep, bool force);
    void read_ep_remote(const VPEDesc &vpe, epid_t ep, void *regs);
    void write_ep_remote(const VPEDesc &vpe, epid_t ep, void *regs);
    void write_ep_local(epid_t ep);

    void recv_msgs(epid_t ep, uintptr_t buf, uint order, uint msgorder);

    void reply(epid_t ep, const void *reply, size_t size, const m3::DTU::Message *msg);

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

    void write_swstate(const VPEDesc &vpe, uint64_t flags, uint64_t notify);
    void write_swflags(const VPEDesc &vpe, uint64_t flags);
    void read_swflags(const VPEDesc &vpe, uint64_t *flags);

private:
#if defined(__gem5__)
    void set_vpeid(const VPEDesc &vpe, vpeid_t id);
    void do_ext_cmd(const VPEDesc &vpe, m3::DTU::reg_t cmd);
    m3::Errors::Code try_ext_cmd(const VPEDesc &vpe, m3::DTU::reg_t cmd);
#endif

    epid_t _ep;
    DTUState _state;
    static DTU _inst;
};

}
