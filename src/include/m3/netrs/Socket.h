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

#include <base/col/List.h>
#include <base/col/Treap.h>

#include <m3/netrs/Net.h>
#include <m3/netrs/NetChannel.h>
#include <m3/session/NetworkManagerRs.h>

namespace m3 {

class SocketRs {
public:
    explicit SocketRs(SocketType ty, NetworkManagerRs &nm, uint8_t protocol);

    // Socket descriptor on the server
    int32_t _sd;
    IpAddr _local_addr;
    uint16_t _local_port;
    IpAddr _remote_addr;
    uint16_t _remote_port;

    // Reference to the network manager
    NetworkManagerRs &_nm;
};

}
