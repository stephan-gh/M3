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
#include <m3/vfs/FileTable.h>

namespace m3 {

UdpSocket::UdpSocket(int sd, capsel_t caps, NetworkManager &nm) : Socket(sd, caps, nm) {
}

UdpSocket::~UdpSocket() {
    remove();
}

FileRef<UdpSocket> UdpSocket::create(NetworkManager &nm, const DgramSocketArgs &args) {
    capsel_t caps;
    int sd = nm.create(SocketType::DGRAM, 0, args, &caps);
    auto sock = std::unique_ptr<UdpSocket>(new UdpSocket(sd, caps, nm));
    return Activity::own().files()->alloc(std::move(sock));
}

void UdpSocket::bind(port_t port) {
    if(_state != Closed)
        throw Exception(Errors::INV_STATE);

    const auto [addr, used_port] = _nm.bind(sd(), port);
    _local_ep.addr = addr;
    _local_ep.port = used_port;
    _state = State::Bound;
}

bool UdpSocket::connect(const Endpoint &ep) {
    if(ep == Endpoint::unspecified())
        throw Exception(Errors::INV_ARGS);

    // connect implicitly calls bind, if not already done, to receive a local ephemeral port
    if(_state != State::Bound)
        bind(0);

    _remote_ep = ep;
    return true;
}

std::optional<size_t> UdpSocket::send(const void *src, size_t amount) {
    return send_to(src, amount, _remote_ep);
}

std::optional<size_t> UdpSocket::send_to(const void *src, size_t amount, const Endpoint &dst_ep) {
    // send_to implicitly calls bind, if not already done, to receive a local ephemeral port
    if(_state != State::Bound)
        bind(0);

    return Socket::do_send(src, amount, dst_ep);
}

std::optional<size_t> UdpSocket::recv(void *dst, size_t amount) {
    if(auto res = recv_from(dst, amount))
        return res.value().first;
    return std::nullopt;
}

std::optional<std::pair<size_t, Endpoint>> UdpSocket::recv_from(void *dst, size_t amount) {
    return Socket::do_recv(dst, amount);
}

void UdpSocket::remove() noexcept {
    tear_down();
}

}
