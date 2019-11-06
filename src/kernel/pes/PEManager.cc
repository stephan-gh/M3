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
#include <base/PEMux.h>

#include "pes/PEManager.h"
#include "pes/VPEManager.h"
#include "DTU.h"
#include "Platform.h"

namespace kernel {

PEManager *PEManager::_inst;

PEManager::PEManager()
    : _muxes(new PEMux*[Platform::pe_count()]),
      _idle_rootpts(new gaddr_t[Platform::pe_count()]) {
    for(peid_t i = Platform::first_pe(); i <= Platform::last_pe(); ++i)
        _muxes[i] = new PEMux(i);
    deprivilege_pes();
}

void PEManager::add_vpe(VPECapability *vpe) {
    _muxes[vpe->obj->peid()]->add_vpe(vpe);
}

void PEManager::remove_vpe(VPE *vpe) {
    _muxes[vpe->peid()]->remove_vpe(vpe);
}

void PEManager::init_vpe(UNUSED VPE *vpe) {
#if defined(__gem5__)
    auto pex = pemux(vpe->peid());
    auto dtustate = pex->dtustate();
    dtustate.reset(0, true);
    vpe->_state = VPE::RUNNING;

    // set address space properties first to load them during the restore
    if(vpe->address_space()) {
        AddrSpace *as = vpe->address_space();
        epid_t rep = Platform::pe(vpe->peid()).has_mmu() ? m3::DTU::PG_REP : 0xFF;
        dtustate.config_pf(as->root_pt(), m3::DTU::PG_SEP, rep);
    }
    dtustate.restore(VPEDesc(vpe->peid(), VPE::INVALID_ID));

    vpe->init_memory();

    start_vpe(vpe);
#endif
}

void PEManager::start_vpe(VPE *vpe) {
#if defined(__host__)
    pemux(vpe->peid())->dtustate().restore(VPEDesc(vpe->peid(), VPE::INVALID_ID));
    vpe->_state = VPE::RUNNING;
    vpe->init_memory();
#else
    uint64_t report = 0;
    uint64_t flags = m3::PEMuxCtrl::WAITING;
    if(vpe->_flags & VPE::F_HASAPP) {
        flags |= m3::PEMuxCtrl::RESTORE;
        flags |= static_cast<uint64_t>(vpe->peid()) << 32;
        flags |= static_cast<uint64_t>(PEMux::VPE_SEL_BEGIN + vpe->id()) << 48;
    }

    DTU::get().write_swstate(vpe->desc(), flags, report);
    DTU::get().inject_irq(vpe->desc());

    while(true) {
        DTU::get().read_swflags(vpe->desc(), &flags);
        if(flags & m3::PEMuxCtrl::SIGNAL)
            break;
    }

    DTU::get().write_swflags(vpe->desc(), 0);
#endif
}

void PEManager::stop_vpe(VPE *vpe) {
    // ensure that all PTEs are in memory
    DTU::get().flush_cache(vpe->desc());

    DTU::get().kill_vpe(vpe->desc(), _idle_rootpts[vpe->peid()]);
    vpe->_flags |= VPE::F_STOPPED;
}

peid_t PEManager::find_pe(const m3::PEDesc &pe, peid_t except) {
    for(peid_t i = Platform::first_pe(); i <= Platform::last_pe(); ++i) {
        if(i != except && !_muxes[i]->used() &&
           Platform::pe(i).isa() == pe.isa() &&
           Platform::pe(i).type() == pe.type())
            return i;
    }
    return 0;
}

void PEManager::deprivilege_pes() {
    for(peid_t i = Platform::first_pe(); i <= Platform::last_pe(); ++i)
        _idle_rootpts[i] = DTU::get().deprivilege(i);
}

}
