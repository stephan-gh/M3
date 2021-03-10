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

#include <base/log/Lib.h>

#include <m3/netrs/Net.h>
#include <m3/netrs/Socket.h>
#include <m3/session/NetworkManagerRs.h>

namespace m3 {

SocketRs::SocketRs(SocketType ty, NetworkManagerRs &nm, uint8_t protocol) : _nm(nm) {
    int32_t sd = nm.create(ty, protocol);
    if(sd < 0) {
        LLOG(NET, "Failed to create socket: Could not allocate socket descriptor!");
        // TODO other error
        throw Exception(Errors::NOT_SUP);
    }

    _sd = sd;
    // Init other parameters that might be set while using this socket.
    _local_addr  = IpAddr(0, 0, 0, 0);
    _local_port  = 0;
    _remote_addr = IpAddr(0, 0, 0, 0);
    _remote_port = 0;
}

}
