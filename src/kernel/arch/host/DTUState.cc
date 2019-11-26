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
#include <base/DTU.h>

#include "pes/VPE.h"
#include "DTUState.h"
#include "DTU.h"

namespace kernel {

void *DTUState::get_ep(epid_t ep) {
    return _regs._eps + ep * m3::DTU::EPS_RCNT;
}

void DTUState::restore(const VPEDesc &) {
    // not supported
}

void DTUState::update_recv(epid_t ep, goff_t base) {
    word_t *regs = reinterpret_cast<word_t*>(get_ep(ep));
    regs[m3::DTU::EP_BUF_ADDR]       += base;
}

void DTUState::invalidate_ep(epid_t ep) {
    word_t *regs = reinterpret_cast<word_t*>(get_ep(ep));
    memset(regs, 0, sizeof(word_t) * m3::DTU::EPS_RCNT);
}

void DTUState::config_recv(epid_t ep, vpeid_t, goff_t buf, uint order, uint msgorder, uint) {
    word_t *regs = reinterpret_cast<word_t*>(get_ep(ep));
    regs[m3::DTU::EP_VALID]          = 1;
    regs[m3::DTU::EP_BUF_ADDR]       = buf;
    regs[m3::DTU::EP_BUF_ORDER]      = static_cast<word_t>(order);
    regs[m3::DTU::EP_BUF_MSGORDER]   = static_cast<word_t>(msgorder);
    regs[m3::DTU::EP_BUF_ROFF]       = 0;
    regs[m3::DTU::EP_BUF_WOFF]       = 0;
    regs[m3::DTU::EP_BUF_MSGCNT]     = 0;
    regs[m3::DTU::EP_BUF_UNREAD]     = 0;
    regs[m3::DTU::EP_BUF_OCCUPIED]   = 0;
}

void DTUState::config_send(epid_t ep, vpeid_t, label_t lbl, peid_t pe, epid_t dstep, uint msgsize, uint credits) {
    word_t *regs = reinterpret_cast<word_t*>(get_ep(ep));
    regs[m3::DTU::EP_VALID]         = 1;
    regs[m3::DTU::EP_LABEL]         = lbl;
    regs[m3::DTU::EP_PEID]          = pe;
    regs[m3::DTU::EP_EPID]          = dstep;
    regs[m3::DTU::EP_CREDITS]       = (1U << msgsize) * credits;
    regs[m3::DTU::EP_MSGORDER]      = msgsize;
}

void DTUState::config_mem(epid_t ep, vpeid_t, peid_t pe, goff_t addr, size_t size, int perms) {
    word_t *regs = reinterpret_cast<word_t*>(get_ep(ep));
    assert((addr & static_cast<goff_t>(perms)) == 0);
    regs[m3::DTU::EP_VALID]         = 1;
    regs[m3::DTU::EP_LABEL]         = addr | static_cast<uint>(perms);
    regs[m3::DTU::EP_PEID]          = pe;
    regs[m3::DTU::EP_EPID]          = 0;
    regs[m3::DTU::EP_CREDITS]       = size;
    regs[m3::DTU::EP_MSGORDER]      = 0;
}

bool DTUState::config_mem_cached(epid_t, peid_t) {
    // unused
    return true;
}

void DTUState::config_pf(gaddr_t, epid_t, epid_t) {
    // not supported
}

}
