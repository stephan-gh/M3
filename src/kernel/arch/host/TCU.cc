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

m3::Errors::Code TCU::inv_reply_remote(peid_t, epid_t, peid_t, epid_t) {
    // unused
    return m3::Errors::NONE;
}

m3::Errors::Code TCU::inval_ep_remote(peid_t pe, epid_t ep, bool, uint32_t *unreadMask) {
    word_t regs[m3::TCU::EPS_RCNT];
    memset(regs, 0, sizeof(regs));
    *unreadMask = 0;
    // TODO detect if credits are outstanding
    write_ep_remote(pe, ep, regs);
    return m3::Errors::NONE;
}

void TCU::write_ep_remote(peid_t pe, epid_t ep, void *regs) {
    uintptr_t eps = static_cast<uintptr_t>(PEManager::get().pemux(pe)->eps_base());
    uintptr_t addr = eps + ep * m3::TCU::EPS_RCNT * sizeof(word_t);
    VPEDesc vpe(pe, VPE::INVALID_ID);
    write_mem(vpe, addr, regs, m3::TCU::EPS_RCNT * sizeof(word_t));
}

void TCU::write_ep_local(epid_t ep) {
    uintptr_t eps = reinterpret_cast<uintptr_t>(m3::TCU::get().ep_regs());
    uintptr_t addr = eps + ep * m3::TCU::EPS_RCNT * sizeof(word_t);
    memcpy(reinterpret_cast<void*>(addr), _state.get_ep(ep), m3::TCU::EPS_RCNT * sizeof(word_t));
}

void TCU::recv_msgs(epid_t ep, uintptr_t buf, uint order, uint msgorder) {
    _state.config_recv(ep, VPE::INVALID_ID, buf, order, msgorder, 0);
    write_ep_local(ep);
}

void TCU::reply(epid_t ep, const void *reply, size_t size, const m3::TCU::Message *msg) {
    m3::TCU::get().reply(ep, reply, size, msg);
}

m3::Errors::Code TCU::send_to(const VPEDesc &vpe, epid_t ep, label_t label, const void *msg,
                              size_t size, label_t replylbl, epid_t replyep) {
    const size_t msg_ord = static_cast<uint>(m3::getnextlog2(size + m3::TCU::HEADER_SIZE));
    m3::TCU::get().configure(TMP_SEP, label, 0, vpe.pe, ep, 1UL << msg_ord, msg_ord);
    return m3::TCU::get().send(TMP_SEP, msg, size, replylbl, replyep);
}

m3::Errors::Code TCU::try_write_mem(const VPEDesc &vpe, goff_t addr, const void *data, size_t size) {
    m3::TCU::get().configure(TMP_MEP, addr, m3::KIF::Perm::RWX, vpe.pe, 0, size, 0);
    m3::TCU::get().write(TMP_MEP, data, size, 0, 0);
    return m3::Errors::NONE;
}

m3::Errors::Code TCU::try_read_mem(const VPEDesc &vpe, goff_t addr, void *data, size_t size) {
    m3::TCU::get().configure(TMP_MEP, addr, m3::KIF::Perm::RWX, vpe.pe, 0, size, 0);
    m3::TCU::get().read(TMP_MEP, data, size, 0, 0);
    return m3::Errors::NONE;
}

void TCU::copy_clear(const VPEDesc &, goff_t, const VPEDesc &, goff_t, size_t, bool) {
    // not supported
}

}
