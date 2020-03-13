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
#include <base/CPU.h>
#include <base/TCU.h>

#include "pes/VPE.h"
#include "pes/VPEManager.h"
#include "TCUState.h"
#include "TCU.h"
#include "Platform.h"

namespace kernel {

void *TCUState::get_ep(epid_t ep) {
    return _regs._eps + ep * m3::TCU::EP_REGS;
}

void TCUState::restore(const VPEDesc &vpe) {
    TCU::get().write_mem(vpe, m3::TCU::MMIO_ADDR, this, sizeof(_regs));
}

void TCUState::invalidate_ep(epid_t ep) {
    m3::TCU::reg_t *r = reinterpret_cast<m3::TCU::reg_t*>(get_ep(ep));
    memset(r, 0, sizeof(m3::TCU::reg_t) * m3::TCU::EP_REGS);
}

void TCUState::config_recv(epid_t ep, vpeid_t vpe, goff_t buf,
                           uint order, uint msgorder, uint reply_eps) {
    m3::TCU::reg_t *r = reinterpret_cast<m3::TCU::reg_t*>(get_ep(ep));
    m3::TCU::reg_t bufSize = static_cast<m3::TCU::reg_t>(order - msgorder);
    m3::TCU::reg_t msgSize = static_cast<m3::TCU::reg_t>(msgorder);
    r[0] = static_cast<m3::TCU::reg_t>(m3::TCU::EpType::RECEIVE) |
            (static_cast<m3::TCU::reg_t>(vpe) << 3) |
            (static_cast<m3::TCU::reg_t>(reply_eps) << 19) |
            (static_cast<m3::TCU::reg_t>(bufSize) << 35) |
            (static_cast<m3::TCU::reg_t>(msgSize) << 41);
    r[1] = buf & 0xFFFFFFFF;
    r[2] = 0;
}

void TCUState::config_send(epid_t ep, vpeid_t vpe, label_t lbl, peid_t pe, epid_t dstep,
                           uint msgorder, uint credits) {
    m3::TCU::reg_t *r = reinterpret_cast<m3::TCU::reg_t*>(get_ep(ep));
    r[0] = static_cast<m3::TCU::reg_t>(m3::TCU::EpType::SEND) |
            (static_cast<m3::TCU::reg_t>(vpe) << 3) |
            (static_cast<m3::TCU::reg_t>(credits) << 19) |
            (static_cast<m3::TCU::reg_t>(credits) << 25) |
            (static_cast<m3::TCU::reg_t>(msgorder) << 31);
    r[1] = (static_cast<m3::TCU::reg_t>(pe & 0xFF) << 16) |
            (static_cast<m3::TCU::reg_t>(dstep & 0xFF) << 0);
    r[2] = lbl;
}

void TCUState::config_mem(epid_t ep, vpeid_t vpe, peid_t pe, goff_t addr, size_t size, uint perm) {
    static_assert(m3::KIF::Perm::R == m3::TCU::R, "TCU::R does not match KIF::Perm::R");
    static_assert(m3::KIF::Perm::W == m3::TCU::W, "TCU::W does not match KIF::Perm::W");

    m3::TCU::reg_t *r = reinterpret_cast<m3::TCU::reg_t*>(get_ep(ep));
    r[0] = static_cast<m3::TCU::reg_t>(m3::TCU::EpType::MEMORY) |
            (static_cast<m3::TCU::reg_t>(vpe) << 3) |
            (static_cast<m3::TCU::reg_t>(perm) << 19) |
            (static_cast<m3::TCU::reg_t>(pe) << 23);
    r[1] = addr;
    r[2] = size;
}

bool TCUState::config_mem_cached(epid_t ep, peid_t pe) {
    m3::TCU::reg_t *r = reinterpret_cast<m3::TCU::reg_t*>(get_ep(ep));
    m3::TCU::reg_t r0, r2;
    r0 = static_cast<m3::TCU::reg_t>(m3::TCU::EpType::MEMORY) |
         (VPE::KERNEL_ID << 3) |
         (pe << 23) |
         (m3::KIF::Perm::RW << 19);
    r2 = 0xFFFFFFFFFFFFFFFF;
    bool res = false;
    if(r0 != r[0]) {
        r[0] = r0;
        res = true;
    }
    if(r[1] != 0) {
        r[1] = 0;
        res = true;
    }
    if(r2 != r[2]) {
        r[2] = r2;
        res = true;
    }
    return res;
}

}
