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

#include <base/Env.h>

#include <m3/com/Semaphore.h>
#include <m3/netrs/UdpSocket.h>
#include <m3/session/NetworkManagerRs.h>
#include <m3/stream/Standard.h>

using namespace m3;

int main() {
    NetworkManagerRs net("net1");
    String status;

    auto socket = UdpSocketRs::create(net);

    // Bind to our address
    socket->bind(IpAddr(192, 168, 112, 1), 1337);

    // notify client
    Semaphore::attach("net").up();

    union {
        uint8_t raw[1024];
        cycles_t time;
    } request;

    IpAddr src_addr;
    uint16_t src_port;

    while(true) {
        ssize_t recv_size = socket->recvfrom(request.raw, sizeof(request.raw), &src_addr, &src_port);
        if (recv_size == -1)
            exitmsg("receive failed");

        // cout << "got package, sending response\n";

        // Send ack
        ssize_t send_size = socket->sendto(request.raw, static_cast<size_t>(recv_size), src_addr, src_port);
        if(send_size == -1)
            exitmsg("send failed");
    }
}
