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
#include <m3/net/RawSocket.h>
#include <m3/session/NetworkManager.h>

namespace m3 {

RawSocket::RawSocket(int sd, capsel_t caps, NetworkManager &nm)
    : Socket(sd, caps, nm) {
}

RawSocket::~RawSocket() {
    tear_down();
}

Reference<RawSocket> RawSocket::create(NetworkManager &nm, uint8_t protocol,
                                       const DgramSocketArgs &args) {
    capsel_t caps;
    int sd = nm.create(SocketType::RAW, protocol, args, &caps);
    auto sock = new RawSocket(sd, caps, nm);
    nm.add_socket(sock);
    return Reference<RawSocket>(sock);
}

ssize_t RawSocket::recv(void *dst, size_t amount) {
    return Socket::do_recv(dst, amount, nullptr);
}

ssize_t RawSocket::send(const void *src, size_t amount) {
    return Socket::do_send(src, amount, Endpoint());
}

}
