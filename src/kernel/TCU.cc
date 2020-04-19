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

#include <base/log/Kernel.h>
#include <base/Env.h>

#include "pes/VPE.h"
#include "TCU.h"

namespace kernel {

static const size_t MAX_RBUFS = 8;

static uintptr_t rbufs[MAX_RBUFS];

void TCU::drop_msgs(epid_t ep, label_t label) {
    size_t rbuf = rbufs[ep];

#if defined(__host__)
    rbuf -= m3::env()->rbuf_start();
#endif

    m3::TCU::get().drop_msgs(rbuf, ep, label);
}

void TCU::recv_msgs(epid_t ep, uintptr_t buf, uint order, uint msgorder) {
    // TODO manage the kernel EPs properly
    static size_t reply_eps = 16;
    assert(ep < MAX_RBUFS);
    rbufs[ep] = buf;

#if defined(__host__)
    buf -= m3::env()->rbuf_start();
#endif

    config_local_ep(ep, [buf, order, msgorder](m3::TCU::reg_t *ep_regs) {
        config_recv(ep_regs, VPE::KERNEL_ID, buf, order, msgorder, reply_eps);
    });

    reply_eps += 1UL << (order - msgorder);
}

const m3::TCU::Message *TCU::fetch_msg(epid_t rep) {
    size_t msg_off = m3::TCU::get().fetch_msg(rep);
    if(msg_off != static_cast<size_t>(-1))
        return reinterpret_cast<const m3::TCU::Message*>(rbufs[rep] + msg_off);
    return nullptr;
}

void TCU::ack_msg(epid_t rep, const m3::TCU::Message *msg) {
    size_t msg_off = reinterpret_cast<uintptr_t>(msg) - rbufs[rep];
    m3::TCU::get().ack_msg(rep, msg_off);
}

void TCU::reply(epid_t ep, const void *reply, size_t size, const m3::TCU::Message *msg) {
    size_t msg_off = reinterpret_cast<uintptr_t>(msg) - rbufs[ep];
    m3::Errors::Code res = m3::TCU::get().reply(ep, reply, size, msg_off);
    if(res != m3::Errors::NONE)
        PANIC("Reply failed");
}

m3::Errors::Code TCU::send_to(peid_t pe, epid_t ep, label_t label, const void *msg,
                              size_t size, label_t replylbl, epid_t replyep) {
    config_local_ep(TMP_SEP, [pe, ep, label](m3::TCU::reg_t *ep_regs) {
        config_send(ep_regs, VPE::KERNEL_ID, label, pe, ep, 0xFFFF, m3::KIF::UNLIM_CREDITS);
    });
    return m3::TCU::get().send(TMP_SEP, msg, size, replylbl, replyep);
}

m3::Errors::Code TCU::try_write_mem(const VPEDesc &vpe, goff_t addr, const void *data, size_t size) {
    config_local_ep(TMP_MEP, [vpe, addr, size](m3::TCU::reg_t *ep_regs) {
        config_mem(ep_regs, VPE::KERNEL_ID, vpe.pe, vpe.id, addr, size, m3::KIF::Perm::W);
    });

    // the kernel can never cause pagefaults with reads/writes
    return m3::TCU::get().write(TMP_MEP, data, size, 0, m3::TCU::CmdFlags::NOPF);
}

m3::Errors::Code TCU::try_read_mem(const VPEDesc &vpe, goff_t addr, void *data, size_t size) {
    config_local_ep(TMP_MEP, [vpe, addr, size](m3::TCU::reg_t *ep_regs) {
        config_mem(ep_regs, VPE::KERNEL_ID, vpe.pe, vpe.id, addr, size, m3::KIF::Perm::R);
    });

    return m3::TCU::get().read(TMP_MEP, data, size, 0, m3::TCU::CmdFlags::NOPF);
}

}
