/*
 * Copyright (C) 2015-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/KIF.h>

#include "TCU.h"

#include <memory>

namespace kernel {

class VPE;

class SyscallHandler {
    SyscallHandler() = delete;

    using handler_func = void (*)(VPE *vpe, const m3::TCU::Message *msg);

public:
    static void init();

    static epid_t ep(size_t no) {
        return TCU::SYSC_REPS + no;
    }

    static epid_t alloc_ep() {
        for(size_t i = 0; i < TCU::SYSC_REP_COUNT; ++i) {
            if(_vpes_per_ep[i] < 32) {
                _vpes_per_ep[i]++;
                return ep(i);
            }
        }
        return EP_COUNT;
    }
    static void free_ep(epid_t id) {
        _vpes_per_ep[id - ep(0)]--;
    }

    static void handle_message(VPE *vpe, const m3::TCU::Message *msg);

private:
    static void create_srv(VPE *vpe, const m3::TCU::Message *msg);
    static void create_sess(VPE *vpe, const m3::TCU::Message *msg);
    static void create_mgate(VPE *vpe, const m3::TCU::Message *msg);
    static void create_rgate(VPE *vpe, const m3::TCU::Message *msg);
    static void create_sgate(VPE *vpe, const m3::TCU::Message *msg);
    static void create_vpe(VPE *vpe, const m3::TCU::Message *msg);
    static void create_map(VPE *vpe, const m3::TCU::Message *msg);
    static void create_sem(VPE *vpe, const m3::TCU::Message *msg);
    static void alloc_ep(VPE *vpe, const m3::TCU::Message *msg);
    static void activate(VPE *vpe, const m3::TCU::Message *msg);
    static void vpe_ctrl(VPE *vpe, const m3::TCU::Message *msg);
    static void vpe_wait(VPE *vpe, const m3::TCU::Message *msg);
    static void derive_mem(VPE *vpe, const m3::TCU::Message *msg);
    static void derive_kmem(VPE *vpe, const m3::TCU::Message *msg);
    static void derive_pe(VPE *vpe, const m3::TCU::Message *msg);
    static void derive_srv(VPE *vpe, const m3::TCU::Message *msg);
    static void get_sess(VPE *vpe, const m3::TCU::Message *msg);
    static void kmem_quota(VPE *vpe, const m3::TCU::Message *msg);
    static void pe_quota(VPE *vpe, const m3::TCU::Message *msg);
    static void sem_ctrl(VPE *vpe, const m3::TCU::Message *msg);
    static void exchange(VPE *vpe, const m3::TCU::Message *msg);
    static void delegate(VPE *vpe, const m3::TCU::Message *msg);
    static void obtain(VPE *vpe, const m3::TCU::Message *msg);
    static void revoke(VPE *vpe, const m3::TCU::Message *msg);
    static void noop(VPE *vpe, const m3::TCU::Message *msg);

    static void add_operation(m3::KIF::Syscall::Operation op, handler_func func) {
        _callbacks[op] = func;
    }

    static void reply_msg(VPE *vpe, const m3::TCU::Message *msg, const void *reply, size_t size);
    static void reply_result(VPE *vpe, const m3::TCU::Message *msg, m3::Errors::Code code);

    static m3::Errors::Code do_exchange(VPE *v1, VPE *v2, const m3::KIF::CapRngDesc &c1,
                                        const m3::KIF::CapRngDesc &c2, bool obtain);
    static void exchange_over_sess(VPE *vpe, const m3::TCU::Message *msg, bool obtain);

    static std::unique_ptr<uint8_t[]> sysc_bufs[TCU::SYSC_REP_COUNT];
    static std::unique_ptr<uint8_t[]> serv_buf;
    static std::unique_ptr<uint8_t[]> pex_buf;
    static ulong _vpes_per_ep[TCU::SYSC_REP_COUNT];
    static handler_func _callbacks[];
};

}
