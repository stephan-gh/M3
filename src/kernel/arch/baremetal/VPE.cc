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

#include "pes/PEManager.h"
#include "pes/VPEManager.h"
#include "pes/VPE.h"
#include "DTU.h"
#include "Platform.h"

namespace kernel {

void VPE::init_eps() {
    auto pemux = PEManager::get().pemux(peid());

    RGateObject rgate(SYSC_MSGSIZE_ORD, SYSC_MSGSIZE_ORD);
    rgate.pe = Platform::kernel_pe();
    rgate.addr = 1;  // has to be non-zero
    rgate.ep = syscall_ep();
    rgate.add_ref(); // don't free this (on destruction of SGateObject)

    // configure syscall endpoint
    SGateObject mobj(&rgate, reinterpret_cast<label_t>(this), 1);
    UNUSED m3::Errors::Code res = pemux->config_snd_ep(m3::DTU::SYSC_SEP, id(), mobj);
    assert(res == m3::Errors::NONE);

    // attach syscall receive endpoint
    rgate.order = m3::nextlog2<SYSC_RBUF_SIZE>::val;
    rgate.msgorder = SYSC_RBUF_ORDER;
    rgate.addr = Platform::def_recvbuf(peid()) + KPEX_RBUF_SIZE + PEXUP_RBUF_SIZE;
    res = pemux->config_rcv_ep(m3::DTU::SYSC_REP, id(), EP_COUNT, rgate);
    assert(res == m3::Errors::NONE);

    // attach upcall receive endpoint
    rgate.order = m3::nextlog2<UPCALL_RBUF_SIZE>::val;
    rgate.msgorder = UPCALL_RBUF_ORDER;
    rgate.addr += SYSC_RBUF_SIZE;
    res = pemux->config_rcv_ep(m3::DTU::UPCALL_REP, id(), m3::DTU::UPCALL_RPLEP, rgate);
    assert(res == m3::Errors::NONE);

    // attach default receive endpoint
    rgate.order = m3::nextlog2<DEF_RBUF_SIZE>::val;
    rgate.msgorder = DEF_RBUF_ORDER;
    rgate.addr += UPCALL_RBUF_SIZE;
    res = pemux->config_rcv_ep(m3::DTU::DEF_REP, id(), EP_COUNT, rgate);
    assert(res == m3::Errors::NONE);

    // TODO don't do that here
    auto size = rgate.addr + (1UL << rgate.order) - Platform::def_recvbuf(peid());
    pemux->set_rbufsize(size);
}

void VPE::finish_start() {
}

}
