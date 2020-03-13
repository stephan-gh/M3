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

#pragma once

#include <base/Common.h>
#include <base/TCU.h>

namespace kernel {

class TCURegs {
public:
    explicit TCURegs()
        : _tcu(),
          _cmd(),
          _eps() {
    }

    m3::TCU::reg_t get(m3::TCU::TCURegs reg) const {
        return _tcu[static_cast<size_t>(reg)];
    }
    void set(m3::TCU::TCURegs reg, m3::TCU::reg_t value) {
        _tcu[static_cast<size_t>(reg)] = value;
    }

    m3::TCU::reg_t _tcu[m3::TCU::TCU_REGS];
    m3::TCU::reg_t _cmd[m3::TCU::CMD_REGS];
    m3::TCU::reg_t _eps[m3::TCU::EP_REGS * EP_COUNT];
} PACKED;

}
