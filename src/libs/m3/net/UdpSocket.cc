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

#include <m3/Exception.h>
#include <m3/net/Socket.h>
#include <m3/net/UdpSocket.h>
#include <m3/session/NetworkManager.h>

namespace m3 {

UdpSocket::UdpSocket(int fd, capsel_t caps, NetworkManager &nm)
    : Socket(fd, caps, nm) {
}

UdpSocket::~UdpSocket() {
    remove();
}

Reference<UdpSocket> UdpSocket::create(NetworkManager &nm, const DgramSocketArgs &args) {
    capsel_t caps;
    int fd = nm.create(SocketType::DGRAM, 0, args, &caps);
    auto sock = new UdpSocket(fd, caps, nm);
    nm.add_socket(sock);
    return Reference<UdpSocket>(sock);
}

void UdpSocket::bind(port_t port) {
    if(_state != Closed)
        throw Exception(Errors::INV_STATE);

    IpAddr addr = _nm.bind(fd(), &port);
    _local_ep.addr = addr;
    _local_ep.port = port;
    _state = State::Bound;
}

void UdpSocket::connect(const Endpoint &ep) {
    if(ep == Endpoint::unspecified())
        throw Exception(Errors::INV_ARGS);

    // connect implicitly calls bind, if not already done, to receive a local ephemeral port
    if(_state != State::Bound)
        bind(0);

    _remote_ep = ep;
}

ssize_t UdpSocket::send(const void *src, size_t amount) {
    return send_to(src, amount, _remote_ep);
}

ssize_t UdpSocket::send_to(const void *src, size_t amount, const Endpoint &dst_ep) {
    // send_to implicitly calls bind, if not already done, to receive a local ephemeral port
    if(_state != State::Bound)
        bind(0);

    return Socket::do_send(src, amount, dst_ep);
}

ssize_t UdpSocket::recv(void *dst, size_t amount) {
    Endpoint _dummy;
    return recv_from(dst, amount, &_dummy);
}

ssize_t UdpSocket::recv_from(void *dst, size_t amount, Endpoint *src_ep) {
    return Socket::do_recv(dst, amount, src_ep);
}

void UdpSocket::remove() noexcept {
    tear_down();
}

}
