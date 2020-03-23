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
#include <base/log/Kernel.h>
#include <base/util/Math.h>
#include <base/CPU.h>
#include <base/TCU.h>

#include "mem/MainMemory.h"
#include "pes/VPEManager.h"
#include "pes/VPE.h"
#include "TCU.h"
#include "Platform.h"

namespace kernel {

static char buffer[8192];

m3::Errors::Code TCU::do_ext_cmd(peid_t pe, m3::TCU::ExtCmdOpCode op, m3::TCU::reg_t *arg) {
    VPEDesc vpe(pe, VPE::INVALID_ID);
    m3::TCU::reg_t reg = static_cast<m3::TCU::reg_t>(op) | *arg << 8;
    m3::CPU::compiler_barrier();
    write_mem(vpe, m3::TCU::priv_reg_addr(m3::TCU::PrivRegs::EXT_CMD), &reg, sizeof(reg));
    read_mem(vpe, m3::TCU::priv_reg_addr(m3::TCU::PrivRegs::EXT_CMD), &reg, sizeof(reg));
    if(arg)
        *arg = reg >> 8;
    return static_cast<m3::Errors::Code>((reg >> 4) & 0xF);
}

void TCU::deprivilege(peid_t pe) {
    VPEDesc vpe(pe, VPE::INVALID_ID);

    // unset the privileged flag
    m3::TCU::reg_t features = 0;
    m3::CPU::compiler_barrier();
    write_mem(vpe, m3::TCU::tcu_reg_addr(m3::TCU::TCURegs::FEATURES), &features, sizeof(features));
}

void TCU::init_vpe(peid_t pe) {
    // flush+invalidate caches to ensure that we have a fresh view on memory. this is required
    // because of the way the pager handles copy-on-write: it reads the current copy from the owner
    // and updates the version in DRAM. for that reason, the cache for new VPEs needs to be clear,
    // so that the cache loads the current version from DRAM.
    m3::TCU::reg_t arg = 1;
    do_ext_cmd(pe, m3::TCU::ExtCmdOpCode::RESET, &arg);
}

void TCU::kill_vpe(peid_t) {
    // nothing to do
}

m3::Errors::Code TCU::inv_reply_remote(peid_t pe, epid_t rep, peid_t rpe, epid_t sep) {
    m3::TCU::reg_t arg = rep | (rpe << 16) | (sep << 24);
    return do_ext_cmd(pe, m3::TCU::ExtCmdOpCode::INV_REPLY, &arg);
}

m3::Errors::Code TCU::inval_ep_remote(peid_t pe, epid_t ep, bool force,
                                      uint32_t *unreadMask) {
    m3::TCU::reg_t arg = ep | (static_cast<m3::TCU::reg_t>(force) << 16);
    m3::Errors::Code res = do_ext_cmd(pe, m3::TCU::ExtCmdOpCode::INV_EP, &arg);
    *unreadMask = arg;
    return res;
}

void TCU::write_ep_remote(peid_t pe, epid_t ep, void *regs) {
    m3::CPU::compiler_barrier();
    VPEDesc vpe(pe, VPE::INVALID_ID);
    write_mem(vpe, m3::TCU::ep_regs_addr(ep), regs, sizeof(m3::TCU::reg_t) * m3::TCU::EP_REGS);
}

void TCU::write_ep_local(epid_t ep) {
    m3::TCU::reg_t *src = reinterpret_cast<m3::TCU::reg_t*>(_state.get_ep(ep));
    m3::TCU::reg_t *dst = reinterpret_cast<m3::TCU::reg_t*>(m3::TCU::ep_regs_addr(ep));
    for(size_t i = 0; i < m3::TCU::EP_REGS; ++i)
        dst[i] = src[i];
}

void TCU::recv_msgs(epid_t ep, uintptr_t buf, uint order, uint msgorder) {
    // TODO manage the kernel EPs properly
    static size_t reply_eps = 16;

    _state.config_recv(ep, VPE::KERNEL_ID, buf, order, msgorder, reply_eps);
    write_ep_local(ep);

    reply_eps += 1UL << (order - msgorder);
}

m3::Errors::Code TCU::send_to(const VPEDesc &vpe, epid_t ep, label_t label, const void *msg,
                              size_t size, label_t replylbl, epid_t replyep) {
    _state.config_send(TMP_SEP, VPE::KERNEL_ID, label, vpe.pe, ep, 0xFFFF, m3::KIF::UNLIM_CREDITS);
    write_ep_local(TMP_SEP);

    return m3::TCU::get().send(TMP_SEP, msg, size, replylbl, replyep);
}

void TCU::reply(epid_t ep, const void *reply, size_t size, const m3::TCU::Message *msg) {
    m3::Errors::Code res = m3::TCU::get().reply(ep, reply, size, msg);
    if(res != m3::Errors::NONE)
        PANIC("Reply failed");
}

m3::Errors::Code TCU::try_write_mem(const VPEDesc &vpe, goff_t addr, const void *data, size_t size) {
    if(_state.config_mem_cached(TMP_MEP, vpe.pe, vpe.id))
        write_ep_local(TMP_MEP);

    // the kernel can never cause pagefaults with reads/writes
    return m3::TCU::get().write(TMP_MEP, data, size, addr, m3::TCU::CmdFlags::NOPF);
}

m3::Errors::Code TCU::try_read_mem(const VPEDesc &vpe, goff_t addr, void *data, size_t size) {
    if(_state.config_mem_cached(TMP_MEP, vpe.pe, vpe.id))
        write_ep_local(TMP_MEP);

    return m3::TCU::get().read(TMP_MEP, data, size, addr, m3::TCU::CmdFlags::NOPF);
}

void TCU::copy_clear(const VPEDesc &dstvpe, goff_t dstaddr,
                     const VPEDesc &srcvpe, goff_t srcaddr,
                     size_t size, bool clear) {
    if(clear)
        memset(buffer, 0, sizeof(buffer));

    size_t rem = size;
    while(rem > 0) {
        size_t amount = m3::Math::min(rem, sizeof(buffer));
        // read it from src, if necessary
        if(!clear)
            TCU::get().read_mem(srcvpe, srcaddr, buffer, amount);
        TCU::get().write_mem(dstvpe, dstaddr, buffer, amount);
        srcaddr += amount;
        dstaddr += amount;
        rem -= amount;
    }
}

}
