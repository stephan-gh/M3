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

#include "lwip/ip_addr.h"
#include "lwip/pbuf.h"
#include "lwip/raw.h"

#include "LwipRawSocket.h"

using namespace m3;

m3::Errors::Code LwipRawSocket::create(uint8_t protocol) {
    // TODO: Validate protocol
    _pcb = raw_new(protocol);
    if(!_pcb) {
        LOG_SOCKET(this, "create failed: allocation of pcb failed");
        return Errors::NO_SPACE;
    }

    return Errors::NONE;
}

ssize_t LwipRawSocket::send_data(const void *, size_t) {
    // TODO: Implement
    return -1;
}

m3::Errors::Code LwipRawSocket::bind(ip4_addr, uint16_t) {
    LOG_SOCKET(this, "bind failed: you can not bind a raw socket");
    return Errors::NOT_SUP;
}

m3::Errors::Code LwipRawSocket::listen() {
    LOG_SOCKET(this, "listen failed: not a stream socket");
    return Errors::NOT_SUP;
}

m3::Errors::Code LwipRawSocket::connect(ip4_addr, uint16_t) {
    LOG_SOCKET(this, "connect failed: you can not connect a raw socket");
    return Errors::NOT_SUP;
}

m3::Errors::Code LwipRawSocket::close() {
    raw_remove(_pcb);
    _pcb = nullptr;
    return Errors::NONE;
}
