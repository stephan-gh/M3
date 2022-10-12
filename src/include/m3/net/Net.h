/*
 * Copyright (C) 2021 Nils Asmussen, Barkhausen Institut
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
 * Copyright (C) 2017, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
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

    void format(OStream &os, const FormatSpecs &) const {
        format_to(os, "IPv4[{}.{}.{}.{}]"_cf, (_addr >> 24) & 0xFF, (_addr >> 16) & 0xFF,
                  (_addr >> 8) & 0xFF, (_addr >> 0) & 0xFF);
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

static inline IStream &operator>>(IStream &is, IpAddr &addr) {
    uint8_t a, b, c, d;
    is >> a;
    if(is.read() != '.')
        throw Exception(Errors::INV_ARGS);
    is >> b;
    if(is.read() != '.')
        throw Exception(Errors::INV_ARGS);
    is >> c;
    if(is.read() != '.')
        throw Exception(Errors::INV_ARGS);
    is >> d;
    addr = IpAddr(a, b, c, d);
    return is;
}

struct Endpoint {
    static Endpoint unspecified() {
        return Endpoint();
    }

    explicit Endpoint() : addr(), port() {
    }
    explicit Endpoint(IpAddr addr, port_t port) : addr(addr), port(port) {
    }

    void format(OStream &os, const FormatSpecs &) const {
        format_to(os, "{}:{}"_cf, addr, port);
    }

    IpAddr addr;
    port_t port;
};

static inline bool operator==(const Endpoint &a, const Endpoint &b) noexcept {
    return a.addr == b.addr && a.port == b.port;
}
static inline bool operator!=(const Endpoint &a, const Endpoint &b) noexcept {
    return !operator==(a, b);
}

}
