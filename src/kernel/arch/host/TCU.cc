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

#include <base/Common.h>

#include "pes/PEManager.h"
#include "pes/VPEManager.h"
#include "TCU.h"

#include <signal.h>

namespace kernel {

static m3::TCU::reg_t all_eps[PE_COUNT][EP_COUNT][m3::TCU::EP_REGS];
static bool dirty_eps[PE_COUNT][EP_COUNT];

void TCU::deprivilege(peid_t) {
    // unsupported
}

void TCU::init_vpe(peid_t) {
    // nothing to do
}

void TCU::kill_vpe(peid_t pe) {
    pid_t pid = VPEManager::get().pid_by_pe(pe);
    // if the VPE didn't run, it has no PID yet
    if(pid != 0)
        kill(pid, SIGKILL);
}

void TCU::config_recv(m3::TCU::reg_t *regs, vpeid_t, goff_t buf, uint order, uint msgorder, uint) {
    regs[m3::TCU::EP_VALID]          = 1;
    regs[m3::TCU::EP_BUF_ADDR]       = buf;
    regs[m3::TCU::EP_BUF_ORDER]      = static_cast<word_t>(order);
    regs[m3::TCU::EP_BUF_MSGORDER]   = static_cast<word_t>(msgorder);
    regs[m3::TCU::EP_BUF_ROFF]       = 0;
    regs[m3::TCU::EP_BUF_WOFF]       = 0;
    regs[m3::TCU::EP_BUF_MSGCNT]     = 0;
    regs[m3::TCU::EP_BUF_UNREAD]     = 0;
    regs[m3::TCU::EP_BUF_OCCUPIED]   = 0;
}

void TCU::config_send(m3::TCU::reg_t *regs, vpeid_t, label_t lbl, peid_t pe, epid_t dstep, uint msgsize, uint credits) {
    regs[m3::TCU::EP_VALID]         = 1;
    regs[m3::TCU::EP_LABEL]         = lbl;
    regs[m3::TCU::EP_PEID]          = pe;
    regs[m3::TCU::EP_EPID]          = dstep;
    if(credits == m3::KIF::UNLIM_CREDITS)
        regs[m3::TCU::EP_CREDITS]       = credits;
    else
        regs[m3::TCU::EP_CREDITS]       = (1U << msgsize) * credits;
    regs[m3::TCU::EP_MSGORDER]      = msgsize;
    regs[m3::TCU::EP_PERM]          = 0;
}

void TCU::config_mem(m3::TCU::reg_t *regs, vpeid_t, peid_t pe, vpeid_t, goff_t addr, size_t size, uint perms) {
    regs[m3::TCU::EP_VALID]         = 1;
    regs[m3::TCU::EP_LABEL]         = addr;
    regs[m3::TCU::EP_PERM]          = static_cast<word_t>(perms);
    regs[m3::TCU::EP_PEID]          = pe;
    regs[m3::TCU::EP_EPID]          = 0;
    regs[m3::TCU::EP_CREDITS]       = size;
    regs[m3::TCU::EP_MSGORDER]      = 0;
}

m3::Errors::Code TCU::inv_reply_remote(peid_t, epid_t, peid_t, epid_t) {
    // unused
    return m3::Errors::NONE;
}

m3::Errors::Code TCU::inval_ep_remote(vpeid_t vpe, peid_t pe, epid_t ep, bool, uint32_t *unreadMask) {
    word_t regs[m3::TCU::EP_REGS];
    memset(regs, 0, sizeof(regs));
    *unreadMask = 0;
    // TODO detect if credits are outstanding
    write_ep_remote(vpe, pe, ep, regs);
    return m3::Errors::NONE;
}

void TCU::write_ep_remote(vpeid_t vpe, peid_t pe, epid_t ep, const void *regs) {
    if(VPEManager::get().vpe(vpe).is_running()) {
        uintptr_t eps = static_cast<uintptr_t>(PEManager::get().pemux(pe)->eps_base());
        uintptr_t addr = eps + ep * m3::TCU::EP_REGS * sizeof(word_t);
        VPEDesc vpe(pe, VPE::INVALID_ID);
        write_mem(vpe, addr, regs, m3::TCU::EP_REGS * sizeof(word_t));
    }
    else {
        memcpy(all_eps[pe][ep], regs, m3::TCU::EP_REGS * sizeof(word_t));
        dirty_eps[pe][ep] = true;
    }
}

void TCU::write_ep_local(epid_t ep, const void *regs) {
    uintptr_t eps = reinterpret_cast<uintptr_t>(m3::TCU::get().ep_regs());
    uintptr_t addr = eps + ep * m3::TCU::EP_REGS * sizeof(word_t);
    memcpy(reinterpret_cast<void*>(addr), regs, m3::TCU::EP_REGS * sizeof(word_t));
}

void TCU::update_eps(vpeid_t vpe, peid_t pe) {
    auto rbuf_base = PEManager::get().pemux(pe)->rbuf_base();
    for(epid_t ep = 0; ep < EP_COUNT; ++ep) {
        if(dirty_eps[pe][ep]) {
            // update base address, now that we now the base
            all_eps[pe][ep][m3::TCU::EP_BUF_ADDR] += rbuf_base;
            // update EP
            write_ep_remote(vpe, pe, ep, all_eps[pe][ep]);
            dirty_eps[pe][ep] = false;
        }
    }
}

void TCU::recv_msgs(epid_t ep, uintptr_t buf, uint order, uint msgorder) {
    config_local_ep(ep, [buf, order, msgorder](m3::TCU::reg_t *ep_regs) {
        config_recv(ep_regs, VPE::KERNEL_ID, buf, order, msgorder, 0);
    });
}

void TCU::reply(epid_t ep, const void *reply, size_t size, const m3::TCU::Message *msg) {
    m3::TCU::get().reply(ep, reply, size, msg);
}

m3::Errors::Code TCU::send_to(peid_t pe, epid_t ep, label_t label, const void *msg,
                              size_t size, label_t replylbl, epid_t replyep) {
    const size_t msg_ord = static_cast<uint>(m3::getnextlog2(size + m3::TCU::HEADER_SIZE));
    config_local_ep(TMP_SEP, [pe, ep, label, msg_ord](m3::TCU::reg_t *ep_regs) {
        config_send(ep_regs, 0, label, pe, ep, msg_ord, 1UL << msg_ord);
    });
    return m3::TCU::get().send(TMP_SEP, msg, size, replylbl, replyep);
}

m3::Errors::Code TCU::try_write_mem(const VPEDesc &vpe, goff_t addr, const void *data, size_t size) {
    config_local_ep(TMP_MEP, [vpe, addr, size](m3::TCU::reg_t *ep_regs) {
        config_mem(ep_regs, 0, vpe.pe, 0, addr, size, m3::KIF::Perm::W);
    });
    m3::TCU::get().write(TMP_MEP, data, size, 0, 0);
    return m3::Errors::NONE;
}

m3::Errors::Code TCU::try_read_mem(const VPEDesc &vpe, goff_t addr, void *data, size_t size) {
    config_local_ep(TMP_MEP, [vpe, addr, size](m3::TCU::reg_t *ep_regs) {
        config_mem(ep_regs, 0, vpe.pe, 0, addr, size, m3::KIF::Perm::R);
    });
    m3::TCU::get().read(TMP_MEP, data, size, 0, 0);
    return m3::Errors::NONE;
}

void TCU::copy_clear(const VPEDesc &, goff_t, const VPEDesc &, goff_t, size_t, bool) {
    // not supported
}

}
