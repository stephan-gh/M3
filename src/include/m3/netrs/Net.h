/*
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

enum SocketType {
    SOCK_STREAM, // TCP
    SOCK_DGRAM,  // UDP
    SOCK_RAW     // IP
};

enum TcpState {
    Closed      = 0,
    Listen      = 1,
    SynSent     = 2,
    SynReceived = 3,
    Established = 4,
    FinWait1    = 5,
    FinWait2    = 6,
    CloseWait   = 7,
    Closing     = 8,
    LastAck     = 9,
    TimeWait    = 10,
    Invalid_Tcp = 11,
};

enum UdpState { Unbound = 0, Open = 1, Invalid_Udp };

/**
 *Contains the anonymous state for some socket type.
 */
struct SocketState {
    /// Returnst the tcp state if this an tcp state, or TcpState::Invalid
    TcpState tcp_state() {
        if(_socket_type == SocketType::SOCK_STREAM) {
            return static_cast<TcpState>(_socket_state);
        }
        else {
            return TcpState::Invalid_Tcp;
        }
    }

    /// Returns the udp state, if this is an udp state
    UdpState udp_state() {
        if(_socket_type == SocketType::SOCK_DGRAM) {
            return static_cast<UdpState>(_socket_state);
        }
        else {
            return UdpState::Invalid_Udp;
        }
    }

    uint64_t _socket_type;
    uint64_t _socket_state;
};

class __attribute__((aligned(4), packed)) IpAddr {
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

namespace net {

/**
 * Represents a MAC address
 */
class MAC {
public:
    static const size_t LEN = 6;

    static MAC broadcast() noexcept {
        return MAC(0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF);
    }

    explicit MAC() noexcept : _bytes() {
    }
    explicit MAC(const uint8_t *b) noexcept : MAC(b[0], b[1], b[2], b[3], b[4], b[5]) {
    }
    explicit MAC(uint8_t b1, uint8_t b2, uint8_t b3, uint8_t b4, uint8_t b5, uint8_t b6) noexcept {
        _bytes[0] = b1;
        _bytes[1] = b2;
        _bytes[2] = b3;
        _bytes[3] = b4;
        _bytes[4] = b5;
        _bytes[5] = b6;
    }

    const uint8_t *bytes() const noexcept {
        return _bytes;
    }
    uint64_t value() const noexcept {
        return (uint64_t)_bytes[5] << 40 | (uint64_t)_bytes[4] << 32 | (uint64_t)_bytes[3] << 24 |
               (uint64_t)_bytes[2] << 16 | (uint64_t)_bytes[1] << 8 | (uint64_t)_bytes[0] << 0;
    }

private:
    uint8_t _bytes[LEN];
};

static inline bool operator==(const MAC &a, const MAC &b) noexcept {
    return a.bytes()[0] == b.bytes()[0] && a.bytes()[1] == b.bytes()[1] && a.bytes()[2] == b.bytes()[2] &&
           a.bytes()[3] == b.bytes()[3] && a.bytes()[4] == b.bytes()[4] && a.bytes()[5] == b.bytes()[5];
}
static inline bool operator!=(const MAC &a, const MAC &b) noexcept {
    return !operator==(a, b);
}

static const int MAX_NETDATA_SIZE = 1024;

/**
 * Represents a network package with context information
 */
struct __attribute__((aligned(2048), packed)) NetData {
    /// Constructs a new NetData package from all context informations
    explicit NetData(int32_t sd, const uint8_t *data, uint32_t data_size, IpAddr src_addr, uint16_t src_port,
                     IpAddr dst_addr, uint16_t dst_port) {
        // Throw an error if the data size is too big
        if(data_size > MAX_NETDATA_SIZE) {
            LLOG(NET, "Packages size was too big when creating NetData. Max size="
                          << MAX_NETDATA_SIZE << ", package size=" << data_size);
            throw Exception(Errors::INV_ARGS);
        }
        // Copy data into local, 0 initialized array
        memcpy(static_cast<void *>(this->data), data, data_size);
        if(data_size < MAX_NETDATA_SIZE) {
            // set a zero byte if this is interpreted as string
            this->data[data_size] = '0';
        }

        this->sd       = sd;
        this->size     = data_size;
        this->src_addr = src_addr;
        this->src_port = src_port;
        this->dst_addr = dst_addr;
        this->dst_port = dst_port;
    }

    /// Initializes an empty package.
    explicit NetData() {
        sd       = 0;
        size     = 0;
        src_addr = IpAddr(0);
        src_port = 0;
        dst_addr = IpAddr(0);
        dst_port = 0;
        memset(static_cast<void *>(data), 0, MAX_NETDATA_SIZE);
    }

    ~NetData() {
        LLOG(NET, "Teddy baer!");
    }

    bool is_empty() {
        if(size == 0) {
            return true;
        }
        else {
            return false;
        }
    }

    /// Returns ptr to inner data that is being transported
    uint8_t *get_data() {
        return data;
    }

    uint32_t get_size() {
        return size;
    }

    size_t send_size() const {
        return 6 * sizeof(uint32_t) + size;
    }

    /**
     * prints the content to LLOG
     */
    void log() {
        LLOG(NET, "sd=" << sd << ", size=" << size << ", src_addr=" << src_addr.addr()
                        << ", src_port=" << src_port << ", dst_addr=" << dst_addr.addr()
                        << ", dst_port=" << dst_port << " data_as_string=" << (char *)data);
    }

    int32_t sd;
    uint32_t size;
    IpAddr src_addr;
    uint16_t src_port;
    uint16_t pad1;
    IpAddr dst_addr;
    uint16_t dst_port;
    uint16_t pad2;
    uint8_t data[MAX_NETDATA_SIZE];
};

}

}
