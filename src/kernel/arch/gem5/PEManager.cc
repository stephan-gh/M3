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
#include "pes/PEMux.h"
#include "pes/VPE.h"
#include "TCU.h"
#include "Platform.h"

namespace kernel {

void PEManager::init_vpe(VPE *vpe) {
    vpe->init_memory();
    if(!Platform::pe(vpe->peid()).is_device())
        vpe->init_eps();
}

void PEManager::start_vpe(VPE *vpe) {
    if(Platform::pe(vpe->peid()).supports_pemux())
        pemux(vpe->peid())->vpe_ctrl(vpe, m3::KIF::PEXUpcalls::VPEOp::VCTRL_START);
}

void PEManager::stop_vpe(VPE *vpe, bool reset) {
    if(Platform::pe(vpe->peid()).supports_pemux() && !(vpe->_flags & VPE::F_STOPPED)) {
        vpe->_flags |= VPE::F_STOPPED;
        pemux(vpe->peid())->vpe_ctrl(vpe, m3::KIF::PEXUpcalls::VPEOp::VCTRL_STOP);
    }

    // reset accelerators to clearly end one run and don't interfere with the next one.
    if(reset && !Platform::pe(vpe->peid()).is_programmable())
        TCU::reset_pe(vpe->peid());
}

}
