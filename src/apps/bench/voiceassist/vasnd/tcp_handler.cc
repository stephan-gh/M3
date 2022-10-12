/*
 * Copyright (C) 2021 Nils Asmussen, Barkhausen Institut
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

#include <m3/com/Semaphore.h>
#include <m3/stream/Standard.h>

#include <endian.h>

#include "handler.h"

using namespace m3;

TCPOpHandler::TCPOpHandler(NetworkManager &nm, m3::IpAddr ip, m3::port_t port)
    : _socket(TcpSocket::create(
          nm, StreamSocketArgs().send_buffer(64 * 1024).recv_buffer(256 * 1024))) {
    _socket->connect(Endpoint(ip, port));
}

void TCPOpHandler::send(const void *data, size_t len) {
    uint64_t length = len;
    if(_socket->send(&length, sizeof(length)).unwrap() != sizeof(length))
        eprintln("send failed"_cf);

    size_t rem = len;
    const char *bytes = static_cast<const char *>(data);
    while(rem > 0) {
        size_t amount = Math::min(rem, static_cast<size_t>(1024));
        if(_socket->send(bytes, amount).unwrap() != amount)
            eprintln("send failed"_cf);

        bytes += amount;
        rem -= amount;
    }

    char dummy;
    if(_socket->recv(&dummy, sizeof(dummy)).unwrap() != sizeof(dummy))
        eprintln("receive failed"_cf);
}
