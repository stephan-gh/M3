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
#include <base/DTU.h>

#include "mem/MainMemory.h"
#include "pes/VPEManager.h"
#include "pes/VPE.h"
#include "DTU.h"
#include "Platform.h"

namespace kernel {

static char buffer[8192];

m3::Errors::Code DTU::do_ext_cmd(const VPEDesc &vpe, m3::DTU::ExtCmdOpCode op, m3::DTU::reg_t *arg) {
    m3::DTU::reg_t reg = static_cast<m3::DTU::reg_t>(op) | *arg << 8;
    m3::CPU::compiler_barrier();
    write_mem(vpe, m3::DTU::priv_reg_addr(m3::DTU::PrivRegs::EXT_CMD), &reg, sizeof(reg));
    read_mem(vpe, m3::DTU::priv_reg_addr(m3::DTU::PrivRegs::EXT_CMD), &reg, sizeof(reg));
    if(arg)
        *arg = reg >> 8;
    return static_cast<m3::Errors::Code>((reg >> 4) & 0xF);
}

void DTU::deprivilege(peid_t pe) {
    VPEDesc vpe(pe, VPE::INVALID_ID);

    // unset the privileged flag
    m3::DTU::reg_t features = 0;
    m3::CPU::compiler_barrier();
    write_mem(vpe, m3::DTU::dtu_reg_addr(m3::DTU::DtuRegs::FEATURES), &features, sizeof(features));
}

void DTU::init_vpe(const VPEDesc &vpe) {
    // flush+invalidate caches to ensure that we have a fresh view on memory. this is required
    // because of the way the pager handles copy-on-write: it reads the current copy from the owner
    // and updates the version in DRAM. for that reason, the cache for new VPEs needs to be clear,
    // so that the cache loads the current version from DRAM.
    m3::DTU::reg_t arg = 1;
    do_ext_cmd(vpe, m3::DTU::ExtCmdOpCode::RESET, &arg);
}

void DTU::kill_vpe(const VPEDesc &vpe) {
    // reset all EPs to remove unread messages
    constexpr size_t userRegs = EP_COUNT - m3::DTU::FIRST_USER_EP;
    constexpr size_t regsSize = (userRegs * m3::DTU::EP_REGS) * sizeof(m3::DTU::reg_t);
    static_assert(regsSize <= sizeof(buffer), "Buffer too small");
    memset(buffer, 0, regsSize);
    write_mem(vpe, m3::DTU::ep_regs_addr(m3::DTU::FIRST_USER_EP), buffer, regsSize);
}

void DTU::flush_cache(const VPEDesc &vpe) {
    m3::DTU::reg_t cmd = static_cast<m3::DTU::reg_t>(m3::DTU::ExtCmdOpCode::FLUSH_CACHE);
    do_ext_cmd(vpe, cmd);
}

m3::Errors::Code DTU::inv_reply_remote(const VPEDesc &vpe, epid_t rep, peid_t pe, epid_t sep) {
    m3::DTU::reg_t arg = rep | (pe << 16) | (sep << 24);
    return do_ext_cmd(vpe, m3::DTU::ExtCmdOpCode::INV_REPLY, &arg);
}

m3::Errors::Code DTU::inval_ep_remote(const kernel::VPEDesc &vpe, epid_t ep, bool force,
                                      uint32_t *unreadMask) {
    m3::DTU::reg_t arg = ep | (static_cast<m3::DTU::reg_t>(force) << 16);
    m3::Errors::Code res = do_ext_cmd(vpe, m3::DTU::ExtCmdOpCode::INV_EP, &arg);
    *unreadMask = arg;
    return res;
}

void DTU::write_ep_remote(const VPEDesc &vpe, epid_t ep, void *regs) {
    m3::CPU::compiler_barrier();
    write_mem(vpe, m3::DTU::ep_regs_addr(ep), regs, sizeof(m3::DTU::reg_t) * m3::DTU::EP_REGS);
}

void DTU::write_ep_local(epid_t ep) {
    m3::DTU::reg_t *src = reinterpret_cast<m3::DTU::reg_t*>(_state.get_ep(ep));
    m3::DTU::reg_t *dst = reinterpret_cast<m3::DTU::reg_t*>(m3::DTU::ep_regs_addr(ep));
    for(size_t i = 0; i < m3::DTU::EP_REGS; ++i)
        dst[i] = src[i];
}

void DTU::recv_msgs(epid_t ep, uintptr_t buf, uint order, uint msgorder) {
    // TODO manage the kernel EPs properly
    static size_t reply_eps = 16;

    _state.config_recv(ep, VPE::KERNEL_ID, buf, order, msgorder, reply_eps);
    write_ep_local(ep);

    reply_eps += 1UL << (order - msgorder);
}

m3::Errors::Code DTU::send_to(const VPEDesc &vpe, epid_t ep, label_t label, const void *msg,
                              size_t size, label_t replylbl, epid_t replyep) {
    _state.config_send(_ep, VPE::KERNEL_ID, label, vpe.pe, ep, 0xFFFF, m3::KIF::UNLIM_CREDITS);
    write_ep_local(_ep);

    return m3::DTU::get().send(_ep, msg, size, replylbl, replyep);
}

void DTU::reply(epid_t ep, const void *reply, size_t size, const m3::DTU::Message *msg) {
    m3::Errors::Code res = m3::DTU::get().reply(ep, reply, size, msg);
    if(res != m3::Errors::NONE)
        PANIC("Reply failed");
}

m3::Errors::Code DTU::try_write_mem(const VPEDesc &vpe, goff_t addr, const void *data, size_t size) {
    if(_state.config_mem_cached(_ep, vpe.pe))
        write_ep_local(_ep);

    // the kernel can never cause pagefaults with reads/writes
    return m3::DTU::get().write(_ep, data, size, addr, m3::DTU::CmdFlags::NOPF);
}

m3::Errors::Code DTU::try_read_mem(const VPEDesc &vpe, goff_t addr, void *data, size_t size) {
    if(_state.config_mem_cached(_ep, vpe.pe))
        write_ep_local(_ep);

    return m3::DTU::get().read(_ep, data, size, addr, m3::DTU::CmdFlags::NOPF);
}

void DTU::copy_clear(const VPEDesc &dstvpe, goff_t dstaddr,
                     const VPEDesc &srcvpe, goff_t srcaddr,
                     size_t size, bool clear) {
    if(clear)
        memset(buffer, 0, sizeof(buffer));

    size_t rem = size;
    while(rem > 0) {
        size_t amount = m3::Math::min(rem, sizeof(buffer));
        // read it from src, if necessary
        if(!clear)
            DTU::get().read_mem(srcvpe, srcaddr, buffer, amount);
        DTU::get().write_mem(dstvpe, dstaddr, buffer, amount);
        srcaddr += amount;
        dstaddr += amount;
        rem -= amount;
    }
}

}
