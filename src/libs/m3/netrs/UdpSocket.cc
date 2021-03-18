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

#include <m3/Exception.h>
#include <m3/netrs/Socket.h>
#include <m3/netrs/UdpSocket.h>
#include <m3/session/NetworkManagerRs.h>

namespace m3 {

UdpSocketRs::UdpSocketRs(int sd, NetworkManagerRs &nm)
    : SocketRs(sd, nm) {
}

UdpSocketRs::~UdpSocketRs() {
    try {
        do_abort(true);
    }
    catch(...) {
        // ignore errors here
    }

    _nm.remove_socket(this);
}

Reference<UdpSocketRs> UdpSocketRs::create(NetworkManagerRs &nm) {
    int sd = nm.create(SOCK_DGRAM, 0);
    auto sock = new UdpSocketRs(sd, nm);
    nm.add_socket(sock);
    return Reference<UdpSocketRs>(sock);
}

void UdpSocketRs::bind(IpAddr addr, uint16_t port) {
    if(_state != Closed)
        inv_state();

    _nm.bind(sd(), addr, port);
    set_local(addr, port, State::Bound);
}

}
