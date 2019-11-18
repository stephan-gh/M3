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
    vpe->_state = VPE::RUNNING;

    // set address space properties first to load them during the restore
    if(vpe->address_space()) {
        AddrSpace *as = vpe->address_space();
        epid_t rep = Platform::pe(vpe->peid()).has_mmu() ? m3::DTU::PG_REP : 0xFF;
        dtustate.config_pf(as->root_pt(), m3::DTU::PG_SEP, rep);
    }
    dtustate.restore(VPEDesc(vpe->peid(), VPE::INVALID_ID));
    DTU::get().init_vpe(vpe->desc());

    vpe->init_memory();

    pemux(vpe->peid())->vpe_ctrl(vpe->id(), m3::KIF::PEXUpcalls::VPEOp::VCTRL_INIT);
#endif
}

void PEManager::start_vpe(VPE *vpe) {
#if defined(__host__)
    pemux(vpe->peid())->dtustate().restore(VPEDesc(vpe->peid(), VPE::INVALID_ID));
    vpe->_state = VPE::RUNNING;
    vpe->init_memory();
#else
    pemux(vpe->peid())->vpe_ctrl(vpe->id(), m3::KIF::PEXUpcalls::VPEOp::VCTRL_START);
#endif
}

void PEManager::stop_vpe(VPE *vpe) {
#if defined(__gem5__)
    // don't do that from the destructor
    if(vpe->state() != VPE::DEAD)
        pemux(vpe->peid())->vpe_ctrl(vpe->id(), m3::KIF::PEXUpcalls::VPEOp::VCTRL_STOP);
#endif

    // ensure that all PTEs are in memory
    DTU::get().flush_cache(vpe->desc());

    DTU::get().kill_vpe(vpe->desc(), _idle_rootpts[vpe->peid()]);
    vpe->_flags |= VPE::F_STOPPED;
}

peid_t PEManager::find_pe(const m3::PEDesc &pe) {
    for(peid_t i = Platform::first_pe(); i <= Platform::last_pe(); ++i) {
        if(Platform::pe(i).isa() == pe.isa() &&
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
