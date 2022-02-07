/*
 * Copyright (C) 2020-2022 Nils Asmussen, Barkhausen Institut
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
    static const uint64_t TILE_SHIFT  = 56;
    static const uint64_t TILE_OFFSET = 0x80;
#else
    static const uint64_t TILE_SHIFT  = 48;
    static const uint64_t TILE_OFFSET = 0;
#endif

public:
    typedef uint64_t raw_t;

    explicit GlobAddr(raw_t raw = 0)
        : _raw(raw) {
    }
    explicit GlobAddr(tileid_t tile, goff_t off)
        : _raw((static_cast<raw_t>(TILE_OFFSET + tile) << TILE_SHIFT) | off) {
    }

    raw_t raw() const {
        return _raw;
    }
    tileid_t tile() const {
        return (_raw >> TILE_SHIFT) - TILE_OFFSET;
    }
    goff_t offset() const {
        return _raw & ((static_cast<goff_t>(1) << TILE_SHIFT) - 1);
    }

    friend void operator+=(GlobAddr &ga, goff_t off) {
        ga._raw += off;
    }
    friend GlobAddr operator+(const GlobAddr &ga, goff_t off) {
        return GlobAddr(ga.tile(), ga.offset() + off);
    }

    friend OStream &operator<<(OStream &os, const GlobAddr &ga) {
        if (ga._raw >= (TILE_OFFSET << TILE_SHIFT))
            os << "G[Tile" << ga.tile() << "+" << fmt(ga.offset(), "#x") << "]";
        // for bootstrap purposes, we need to use global addresses without Tile prefix
        else
            os << "G[" << fmt(ga.raw(), "#x") << "]";
        return os;
    }

private:
    raw_t _raw;
};

}
