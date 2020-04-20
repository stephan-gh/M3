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

void TCU::init_vpe(peid_t) {
    // nothing tod o
}

void TCU::kill_vpe(peid_t) {
    // nothing to do
}

void TCU::config_recv(m3::TCU::reg_t *r, vpeid_t vpe, goff_t buf, uint order,
                      uint msgorder, uint reply_eps) {
    m3::TCU::reg_t bufSize = static_cast<m3::TCU::reg_t>(order - msgorder);
    m3::TCU::reg_t msgSize = static_cast<m3::TCU::reg_t>(msgorder);
    r[0] = static_cast<m3::TCU::reg_t>(m3::TCU::EpType::RECEIVE) |
            (static_cast<m3::TCU::reg_t>(vpe) << 3) |
            (static_cast<m3::TCU::reg_t>(reply_eps) << 19) |
            (static_cast<m3::TCU::reg_t>(bufSize) << 35) |
            (static_cast<m3::TCU::reg_t>(msgSize) << 41);
    r[1] = buf;
    r[2] = 0;
}

void TCU::config_send(m3::TCU::reg_t *r, vpeid_t vpe, label_t lbl, peid_t pe, epid_t dstep,
                      uint msgorder, uint credits) {
    r[0] = static_cast<m3::TCU::reg_t>(m3::TCU::EpType::SEND) |
            (static_cast<m3::TCU::reg_t>(vpe) << 3) |
            (static_cast<m3::TCU::reg_t>(credits) << 19) |
            (static_cast<m3::TCU::reg_t>(credits) << 25) |
            (static_cast<m3::TCU::reg_t>(msgorder) << 31);
    r[1] = (static_cast<m3::TCU::reg_t>(pe & 0xFF) << 16) |
            (static_cast<m3::TCU::reg_t>(dstep & 0xFF) << 0);
    r[2] = lbl;
}

void TCU::config_mem(m3::TCU::reg_t *r, vpeid_t vpe, peid_t pe, vpeid_t tvpe, goff_t addr,
                     size_t size, uint perm) {
    static_assert(m3::KIF::Perm::R == m3::TCU::R, "TCU::R does not match KIF::Perm::R");
    static_assert(m3::KIF::Perm::W == m3::TCU::W, "TCU::W does not match KIF::Perm::W");

    r[0] = static_cast<m3::TCU::reg_t>(m3::TCU::EpType::MEMORY) |
            (static_cast<m3::TCU::reg_t>(vpe) << 3) |
            (static_cast<m3::TCU::reg_t>(perm) << 19) |
            (static_cast<m3::TCU::reg_t>(pe) << 23) |
            (static_cast<m3::TCU::reg_t>(tvpe) << 31);
    r[1] = addr;
    r[2] = size;
}

m3::Errors::Code TCU::inv_reply_remote(peid_t pe, epid_t rep, peid_t rpe, epid_t sep) {
    m3::TCU::reg_t arg = rep | (rpe << 16) | (sep << 24);
    return do_ext_cmd(pe, m3::TCU::ExtCmdOpCode::INV_REPLY, &arg);
}

m3::Errors::Code TCU::inval_ep_remote(vpeid_t, peid_t pe, epid_t ep, bool force, uint32_t *unreadMask) {
    m3::TCU::reg_t arg = ep | (static_cast<m3::TCU::reg_t>(force) << 16);
    m3::Errors::Code res = do_ext_cmd(pe, m3::TCU::ExtCmdOpCode::INV_EP, &arg);
    *unreadMask = arg;
    return res;
}

void TCU::write_ep_remote(vpeid_t, peid_t pe, epid_t ep, const void *regs) {
    m3::CPU::compiler_barrier();
    VPEDesc vpe(pe, VPE::INVALID_ID);
    write_mem(vpe, m3::TCU::ep_regs_addr(ep), regs, sizeof(m3::TCU::reg_t) * m3::TCU::EP_REGS);
}

void TCU::write_ep_local(epid_t ep, const void *regs) {
    const m3::TCU::reg_t *src = reinterpret_cast<const m3::TCU::reg_t*>(regs);
    uintptr_t base = m3::TCU::ep_regs_addr(ep);
    for(size_t i = 0; i < m3::TCU::EP_REGS; ++i)
        m3::CPU::write8b(base + i *sizeof(m3::TCU::reg_t), src[i]);
}

void TCU::update_eps(vpeid_t, peid_t) {
    // nothing to do
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
            read_mem(srcvpe, srcaddr, buffer, amount);
        write_mem(dstvpe, dstaddr, buffer, amount);
        srcaddr += amount;
        dstaddr += amount;
        rem -= amount;
    }
}

}
