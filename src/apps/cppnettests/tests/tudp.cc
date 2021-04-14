/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/Common.h>

#include <m3/com/Semaphore.h>
#include <m3/netrs/UdpSocket.h>
#include <m3/session/NetworkManagerRs.h>
#include <m3/Test.h>

#include "../cppnettests.h"

using namespace m3;

static void basics() {
    NetworkManagerRs net("net0");

    auto socket = UdpSocketRs::create(net);

    WVASSERTEQ(socket->state(), SocketRs::Closed);

    socket->bind(2000);
    WVASSERTEQ(socket->state(), SocketRs::Bound);

    WVASSERTERR(Errors::INV_STATE, [&socket] {
        socket->bind(2001);
    });

    socket->abort();
    WVASSERTEQ(socket->state(), SocketRs::Closed);
}

NOINLINE static void data() {
    NetworkManagerRs net("net0");

    auto socket = UdpSocketRs::create(net);
    socket->bind(2001);

    IpAddr dest_addr = IpAddr(192, 168, 112, 1);
    port_t dest_port = 1337;
    IpAddr src_addr;
    port_t src_port;

    uint8_t send_buf[1024];
    for(int i = 0; i < 1024; ++i)
        send_buf[i] = i;

    uint8_t recv_buf[1024];

    size_t packet_sizes[] = {8, 16, 32, 64, 128, 256, 512, 1024};

    for(auto pkt_size : packet_sizes) {
        socket->send_to(send_buf, pkt_size, dest_addr, dest_port);

        ssize_t recv_size = socket->recv_from(recv_buf, sizeof(recv_buf), &src_addr, &src_port);

        WVASSERTEQ(static_cast<ssize_t>(pkt_size), recv_size);
        WVASSERTEQ(src_addr.addr(), dest_addr.addr());
        WVASSERTEQ(src_port, dest_port);

        for(ssize_t i = 0; i < recv_size; ++i)
            WVASSERTEQ(recv_buf[i], send_buf[i]);
    }
}

void tudp() {
    // wait for UDP socket just once
    Semaphore::attach("net-udp").down();

    RUN_TEST(basics);
    RUN_TEST(data);
}
