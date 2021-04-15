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
#include <m3/net/Socket.h>
#include <m3/net/UdpSocket.h>
#include <m3/session/NetworkManager.h>

namespace m3 {

UdpSocket::UdpSocket(int sd, capsel_t caps, NetworkManager &nm)
    : Socket(sd, caps, nm) {
}

UdpSocket::~UdpSocket() {
    try {
        do_abort(true);
    }
    catch(...) {
        // ignore errors here
    }

    _nm.remove_socket(this);
}

Reference<UdpSocket> UdpSocket::create(NetworkManager &nm, const DgramSocketArgs &args) {
    capsel_t caps;
    int sd = nm.create(SocketType::DGRAM, 0, args, &caps);
    auto sock = new UdpSocket(sd, caps, nm);
    nm.add_socket(sock);
    return Reference<UdpSocket>(sock);
}

void UdpSocket::bind(port_t port) {
    if(_state != Closed)
        throw Exception(Errors::INV_STATE);

    IpAddr addr = _nm.bind(sd(), port);
    set_local(addr, port, State::Bound);
}

ssize_t UdpSocket::recv_from(void *dst, size_t amount, IpAddr *src_addr, port_t *src_port) {
    return Socket::do_recv(dst, amount, src_addr, src_port);
}

ssize_t UdpSocket::send_to(const void *src, size_t amount, IpAddr dst_addr, port_t dst_port) {
    return Socket::do_send(src, amount, dst_addr, dst_port);
}

}
