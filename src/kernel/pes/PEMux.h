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

#include "DTUState.h"

namespace kernel {

class VPE;

class PEMux {
public:
    explicit PEMux(peid_t pe)
        : _vpes(), _pe(pe), _headers(), _dtustate() {
    }

    bool used() const {
        return _vpes > 0;
    }
    void add_vpe(VPE *) {
        assert(_vpes == 0);
        _vpes++;
    }
    void remove_vpe(VPE *) {
        _vpes--;
        _headers = 0;
    }

    size_t headers() const {
        return _headers;
    }

    DTUState &dtustate() {
        return _dtustate;
    }

    size_t allocate_headers(size_t num) {
        // TODO really manage the header space and zero the headers first in case they are reused
        if(_headers + num > m3::DTU::HEADER_COUNT)
            return m3::DTU::HEADER_COUNT;
        _headers += num;
        return _headers - num;
    }

    void invalidate_eps() {
        // no update on the PE here, since we don't save the state anyway
        _dtustate.invalidate_eps(m3::DTU::FIRST_FREE_EP);
    }

private:
    size_t _vpes;
    peid_t _pe;
    size_t _headers;
    DTUState _dtustate;
};

}
