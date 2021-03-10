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

#include <m3/netrs/Socket.h>
#include <m3/session/NetworkManagerRs.h>

namespace m3 {

class UdpSocketRs {
public:
    explicit UdpSocketRs(NetworkManagerRs &nm);
    void bind(IpAddr addr, uint16_t port);
    /**
     *Returns a net data package. It the socket is not in blocking mode, the package might be empty.
     */
    m3::net::NetData recv();
    void send(IpAddr dest_addr, uint16_t dest_port, uint8_t *data, uint32_t size);
    UdpState state();
    void set_blocking(bool should_block);

private:
    bool _is_blocking;
    SocketRs _socket;
};

}
