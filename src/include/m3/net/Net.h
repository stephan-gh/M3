/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
 * Copyright (C) 2017, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
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
#include <base/col/SList.h>
#include <base/log/Lib.h>
#include <base/util/String.h>

#include <m3/Exception.h>

namespace m3 {

using port_t = uint16_t;

enum class SocketType {
    STREAM, // TCP
    DGRAM,  // UDP
    RAW     // IP
};

class IpAddr {
public:
    explicit IpAddr() noexcept : _addr(0) {
    }

    explicit IpAddr(uint32_t addr) noexcept : _addr(addr) {
    }
    explicit IpAddr(uint8_t a, uint8_t b, uint8_t c, uint8_t d) noexcept
        : _addr(static_cast<uint32_t>(a << 24 | b << 16 | c << 8 | d)) {
    }

    uint32_t addr() const noexcept {
        return _addr;
    }

    void addr(uint32_t addr) noexcept {
        _addr = addr;
    }

private:
    uint32_t _addr;
};

static inline bool operator==(const IpAddr &a, const IpAddr &b) noexcept {
    return a.addr() == b.addr();
}
static inline bool operator!=(const IpAddr &a, const IpAddr &b) noexcept {
    return !operator==(a, b);
}

static inline OStream &operator<<(OStream &os, const IpAddr &a) noexcept {
    os << "Ipv4[" << ((a.addr() >> 24) & 0xFF) << "."
                  << ((a.addr() >> 16) & 0xFF) << "."
                  << ((a.addr() >> 8) & 0xFF) << "."
                  << ((a.addr() >> 0) & 0xFF) << "]";
    return os;
}

}
