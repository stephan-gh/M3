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
#include <base/PEMux.h>

#include "mem/MainMemory.h"
#include "pes/VPEManager.h"
#include "pes/VPE.h"
#include "DTU.h"
#include "Platform.h"

namespace kernel {

static char buffer[4096];

void DTU::do_ext_cmd(const VPEDesc &vpe, m3::DTU::reg_t cmd) {
    m3::Errors::Code res = try_ext_cmd(vpe, cmd);
    if(res != m3::Errors::NONE)
        PANIC("External command " << cmd << " failed: " << res);
}

m3::Errors::Code DTU::try_ext_cmd(const VPEDesc &vpe, m3::DTU::reg_t cmd) {
    m3::DTU::reg_t reg = cmd;
    m3::CPU::compiler_barrier();
    return try_write_mem(vpe, m3::DTU::dtu_reg_addr(m3::DTU::DtuRegs::EXT_CMD), &reg, sizeof(reg));
}

gaddr_t DTU::deprivilege(peid_t pe) {
    VPEDesc vpe(pe, VPE::INVALID_ID);

    // remember root PT
    gaddr_t idle_rootpt;
    read_mem(vpe, m3::DTU::dtu_reg_addr(m3::DTU::DtuRegs::ROOT_PT), &idle_rootpt, sizeof(idle_rootpt));

    // unset the privileged flag
    m3::DTU::reg_t features = 0;
    m3::CPU::compiler_barrier();
    write_mem(vpe, m3::DTU::dtu_reg_addr(m3::DTU::DtuRegs::FEATURES), &features, sizeof(features));
    return idle_rootpt;
}

cycles_t DTU::get_time() {
    return m3::DTU::get().tsc();
}

void DTU::kill_vpe(const VPEDesc &vpe, gaddr_t idle_rootpt) {
    ext_request(vpe, m3::DTU::ExtReqOpCode::STOP);

    // reset all EPs to remove unread messages
    size_t regsSize = (TOTAL_EPS - m3::DTU::FIRST_USER_EP) * m3::DTU::EP_REGS;
    regsSize *= sizeof(m3::DTU::reg_t);
    memset(buffer, 0, regsSize);
    write_mem(vpe, m3::DTU::ep_regs_addr(m3::DTU::FIRST_USER_EP), buffer, regsSize);
    // reset events register to be sure that the remote core can sleep
    write_mem(vpe, m3::DTU::dtu_reg_addr(m3::DTU::DtuRegs::EVENTS), buffer, sizeof(m3::DTU::reg_t));

    // set new root PT and disable pagefaults
    static_assert(static_cast<int>(m3::DTU::DtuRegs::FEATURES) == 0, "FEATURES illdefined");
    static_assert(static_cast<int>(m3::DTU::DtuRegs::ROOT_PT) == 1, "ROOT_PT illdefined");
    static_assert(static_cast<int>(m3::DTU::DtuRegs::PF_EP) == 2, "PF_EP illdefined");
    m3::DTU::reg_t regs[3] = {0, idle_rootpt, 0};
    m3::CPU::compiler_barrier();
    write_mem(vpe, m3::DTU::dtu_reg_addr(m3::DTU::DtuRegs::FEATURES), regs, sizeof(regs));
}

void DTU::wakeup(const VPEDesc &vpe) {
    m3::DTU::reg_t cmd = static_cast<m3::DTU::reg_t>(m3::DTU::ExtCmdOpCode::WAKEUP_CORE);
    do_ext_cmd(vpe, cmd);
}

void DTU::flush_cache(const VPEDesc &vpe) {
    m3::DTU::reg_t cmd = static_cast<m3::DTU::reg_t>(m3::DTU::ExtCmdOpCode::FLUSH_CACHE);
    do_ext_cmd(vpe, cmd);
}

void DTU::inject_irq(const VPEDesc &vpe) {
    ext_request(vpe, m3::DTU::ExtReqOpCode::PEMUX);
}

void DTU::ext_request(const VPEDesc &vpe, uint64_t req) {
    m3::DTU::reg_t reg = req;
    m3::CPU::compiler_barrier();
    write_mem(vpe, m3::DTU::dtu_reg_addr(m3::DTU::ReqRegs::EXT_REQ), &reg, sizeof(reg));
}

void DTU::invtlb_remote(const VPEDesc &vpe) {
    do_ext_cmd(vpe, static_cast<m3::DTU::reg_t>(m3::DTU::ExtCmdOpCode::INV_TLB));
}

void DTU::invlpg_remote(const VPEDesc &vpe, goff_t virt) {
    assert((virt & PAGE_MASK) == 0);
    do_ext_cmd(vpe, static_cast<m3::DTU::reg_t>(m3::DTU::ExtCmdOpCode::INV_PAGE) | (virt << 4));
}

m3::Errors::Code DTU::inv_reply_remote(const VPEDesc &vpe, epid_t rep, peid_t pe, epid_t sep) {
    m3::DTU::reg_t cmd = static_cast<m3::DTU::reg_t>(m3::DTU::ExtCmdOpCode::INV_REPLY);
    cmd |= (rep << 4) | (pe << 12) | (sep << 20);
    return try_ext_cmd(vpe, cmd);
}

m3::Errors::Code DTU::inval_ep_remote(const kernel::VPEDesc &vpe, epid_t ep, bool force) {
    m3::DTU::reg_t cmd =
        static_cast<m3::DTU::reg_t>(m3::DTU::ExtCmdOpCode::INV_EP) | (ep << 4) |
        (static_cast<m3::DTU::reg_t>(force) << 12);
    return try_ext_cmd(vpe, cmd);
}

void DTU::read_ep_remote(const VPEDesc &vpe, epid_t ep, void *regs) {
    m3::CPU::compiler_barrier();
    read_mem(vpe, m3::DTU::ep_regs_addr(ep), regs, sizeof(m3::DTU::reg_t) * m3::DTU::EP_REGS);
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
    static size_t reply_eps = EP_COUNT;

    _state.config_recv(ep, buf, order, msgorder, reply_eps);
    write_ep_local(ep);

    reply_eps += 1UL << (order - msgorder);
}

m3::Errors::Code DTU::send_to(const VPEDesc &vpe, epid_t ep, label_t label, const void *msg,
                              size_t size, label_t replylbl, epid_t replyep) {
    _state.config_send(_ep, label, vpe.pe, ep, 0xFFFF, m3::KIF::UNLIM_CREDITS);
    write_ep_local(_ep);

    m3::DTU::get().write_reg(m3::DTU::CmdRegs::DATA, reinterpret_cast<m3::DTU::reg_t>(msg) |
        (static_cast<m3::DTU::reg_t>(size) << 48));
    m3::DTU::get().write_reg(m3::DTU::CmdRegs::REPLY_LABEL, replylbl);
    m3::CPU::compiler_barrier();
    m3::DTU::reg_t cmd = m3::DTU::get().build_command(_ep, m3::DTU::CmdOpCode::SEND, 0, replyep);
    m3::DTU::get().write_reg(m3::DTU::CmdRegs::COMMAND, cmd);

    return m3::DTU::get().get_error();
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

void DTU::write_swstate(const VPEDesc &vpe, uint64_t flags, uint64_t notify) {
    if(!Platform::pe(vpe.pe).supports_ctx())
        return;
    uint64_t vals[2] = {notify, flags};
    write_mem(vpe, PEMUX_YIELD, &vals, sizeof(vals));
}

void DTU::write_swflags(const VPEDesc &vpe, uint64_t flags) {
    if(!Platform::pe(vpe.pe).supports_ctx())
        return;
    write_mem(vpe, PEMUX_FLAGS, &flags, sizeof(flags));
}

void DTU::read_swflags(const VPEDesc &vpe, uint64_t *flags) {
    if(!Platform::pe(vpe.pe).supports_ctx()) {
        *flags = m3::PEMuxCtrl::SIGNAL;
        return;
    }
    read_mem(vpe, PEMUX_FLAGS, flags, sizeof(*flags));
}

}
