/*
 * Copyright (C) 2016, Nils Asmussen <nils@os.inf.tu-dresden.de>
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
#include <base/stream/OStream.h>

namespace m3 {

class GlobAddr {
#if defined(__gem5__)
    static const uint64_t PE_SHIFT  = 56;
    static const uint64_t PE_OFFSET = 0x80;
#else
    static const uint64_t PE_SHIFT  = 48;
    static const uint64_t PE_OFFSET = 0;
#endif

public:
    typedef uint64_t raw_t;

    explicit GlobAddr(raw_t raw = 0)
        : _raw(raw) {
    }
    explicit GlobAddr(peid_t pe, goff_t off)
        : _raw((static_cast<raw_t>(PE_OFFSET + pe) << PE_SHIFT) | off) {
    }

    raw_t raw() const {
        return _raw;
    }
    peid_t pe() const {
        return (_raw >> PE_SHIFT) - PE_OFFSET;
    }
    goff_t offset() const {
        return _raw & ((static_cast<goff_t>(1) << PE_SHIFT) - 1);
    }

    friend void operator+=(GlobAddr &ga, goff_t off) {
        ga._raw += off;
    }
    friend GlobAddr operator+(const GlobAddr &ga, goff_t off) {
        return GlobAddr(ga.pe(), ga.offset() + off);
    }

    friend OStream &operator<<(OStream &os, const GlobAddr &ga) {
        if (ga._raw >= (PE_OFFSET << PE_SHIFT))
            os << "G[PE" << ga.pe() << "+" << fmt(ga.offset(), "#x") << "]";
        // for bootstrap purposes, we need to use global addresses without PE prefix
        else
            os << "G[" << fmt(ga.raw(), "#x") << "]";
        return os;
    }

private:
    raw_t _raw;
};

}