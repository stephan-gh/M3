/*
 * Copyright (C) 2018, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
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

#include <m3/net/RawSocket.h>
#include <m3/session/NetworkManager.h>

namespace m3 {

RawSocket::RawSocket(int sd, NetworkManager& nm)
    : Socket(sd, nm)
{
}

RawSocket::~RawSocket() {
}

Socket::SocketType RawSocket::type() {
    return SOCK_RAW;
}

Errors::Code RawSocket::bind(IpAddr , uint16_t) {
    return Errors::NOT_SUP;
}

ssize_t RawSocket::sendto(const void *, size_t, IpAddr, uint16_t) {
    Errors::last = Errors::NOT_SUP;
    return -1;
}

ssize_t RawSocket::recvmsg(void *, size_t, IpAddr *, uint16_t *) {
    Errors::last = Errors::NOT_SUP;
    return -1;
}

}
