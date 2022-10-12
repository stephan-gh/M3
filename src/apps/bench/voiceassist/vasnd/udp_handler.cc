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

UDPOpHandler::UDPOpHandler(NetworkManager &nm, m3::IpAddr ip, m3::port_t port)
    : _ep(ip, port),
      _socket(UdpSocket::create(nm, DgramSocketArgs().send_buffer(8, 8 * 1024))) {
}

void UDPOpHandler::send(const void *data, size_t len) {
    size_t rem = len;
    const char *bytes = static_cast<const char *>(data);
    while(rem > 0) {
        size_t amount = Math::min(rem, static_cast<size_t>(512));
        if(_socket->send_to(bytes, amount, _ep).unwrap() != amount)
            eprintln("send failed"_cf);

        bytes += amount;
        rem -= amount;
    }
}
