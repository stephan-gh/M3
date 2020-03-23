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
#include "TCU.h"
#include "Platform.h"

namespace kernel {

PEManager *PEManager::_inst;

PEManager::PEManager()
    : _muxes(new PEMux*[Platform::pe_count()]) {
    deprivilege_pes();
    for(peid_t i = Platform::first_pe(); i <= Platform::last_pe(); ++i)
        _muxes[i] = new PEMux(i);
}

void PEManager::add_vpe(VPECapability *vpe) {
    _muxes[vpe->obj->peid()]->add_vpe(vpe);
}

void PEManager::remove_vpe(VPE *vpe) {
    _muxes[vpe->peid()]->remove_vpe(vpe);
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
        TCU::deprivilege(i);
}

}
