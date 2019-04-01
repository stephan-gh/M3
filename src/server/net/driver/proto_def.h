/*
 * Copyright (C) 2019, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
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

namespace net {

static const size_t ETH_HWADDR_LEN = 6;

struct eth_addr {
    uint8_t addr[ETH_HWADDR_LEN];
} PACKED;

struct eth_hdr {
    eth_addr dest;
    eth_addr src;
    uint16_t type;
} PACKED;

static const uint16_t ETHTYPE_IP = 0x0008;

struct ip4_addr {
    uint32_t addr;
} PACKED;

struct ip_hdr {
    uint8_t v_hl;
    uint8_t tos;
    uint16_t len;
    uint16_t id;
    uint16_t offset;
    uint8_t ttl;
    uint8_t proto;
    uint16_t chksum;
    ip4_addr src;
    ip4_addr dest;
} PACKED;

static const uint8_t IP_PROTO_UDP = 17;
static const uint8_t IP_PROTO_TCP = 6;

static const uint8_t TCP_CHECKSUM_OFFSET = 0x10;
static const uint8_t UDP_CHECKSUM_OFFSET = 0x06;

}
