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

#include <base/Common.h>

#include <m3/Test.h>
#include <m3/com/Semaphore.h>
#include <m3/net/UdpSocket.h>
#include <m3/session/NetworkManager.h>
#include <m3/vfs/Waiter.h>

#include "../cppnettests.h"

using namespace m3;

static void basics() {
    NetworkManager net("net0");

    auto socket = UdpSocket::create(net);

    WVASSERTEQ(socket->state(), Socket::Closed);
    WVASSERTEQ(socket->local_endpoint(), Endpoint::unspecified());

    socket->bind(2000);
    WVASSERTEQ(socket->state(), Socket::Bound);
    WVASSERTEQ(socket->local_endpoint(), Endpoint(IpAddr(192, 168, 112, 2), 2000));

    WVASSERTERR(Errors::INV_STATE, [&socket] {
        socket->bind(2001);
    });
}

static void connect() {
    NetworkManager net("net0");

    auto socket = UdpSocket::create(net);

    WVASSERTEQ(socket->state(), Socket::Closed);
    WVASSERTEQ(socket->local_endpoint(), Endpoint::unspecified());

    socket->connect(Endpoint(IpAddr(192, 168, 112, 1), 1337));
    WVASSERTEQ(socket->state(), Socket::Bound);
}

static ssize_t send_recv(FileWaiter &waiter, FileRef<UdpSocket> &socket, const Endpoint &dest,
                         const uint8_t *send_buf, size_t sbuf_size, TimeDuration timeout,
                         uint8_t *recv_buf, size_t rbuf_size, Endpoint *src) {
    socket->send_to(send_buf, sbuf_size, dest);

    waiter.wait_for(timeout);

    if(socket->has_data())
        return socket->recv_from(recv_buf, rbuf_size, src);
    return 0;
}

NOINLINE static void data() {
    const TimeDuration TIMEOUT = TimeDuration::from_secs(1);

    NetworkManager net("net0");

    auto socket = UdpSocket::create(net);
    socket->set_blocking(false);

    Endpoint src;
    Endpoint dest = Endpoint(IpAddr(192, 168, 112, 1), 1337);

    uint8_t send_buf[1024];
    for(int i = 0; i < 1024; ++i)
        send_buf[i] = i;

    uint8_t recv_buf[1024];

    FileWaiter waiter;
    waiter.add(socket->fd(), File::INPUT);

    // do one initial send-receive with a higher timeout than the smoltcp-internal timeout to
    // workaround the high ARP-request delay with the loopback device.
    send_recv(waiter, socket, dest, send_buf, 1, TimeDuration::from_secs(6), recv_buf,
              sizeof(recv_buf), &src);

    size_t packet_sizes[] = {8, 16, 32, 64, 128, 256, 512, 1024};

    WVASSERTERR(Errors::OUT_OF_BOUNDS, [&socket, &dest] {
        char tmp[4096];
        static_assert(sizeof(tmp) > NetEventChannel::MAX_PACKET_SIZE, "Packet too small");
        socket->send_to(tmp, sizeof(tmp), dest);
    });
    WVASSERTERR(Errors::OUT_OF_BOUNDS, [&socket, &dest] {
        char tmp[4096];
        socket->send_to(tmp, NetEventChannel::MAX_PACKET_SIZE + 1, dest);
    });

    for(auto pkt_size : packet_sizes) {
        while(true) {
            ssize_t recv_size = send_recv(waiter, socket, dest, send_buf, pkt_size, TIMEOUT,
                                          recv_buf, sizeof(recv_buf), &src);
            if(recv_size != 0) {
                WVASSERTEQ(static_cast<ssize_t>(pkt_size), recv_size);
                WVASSERTEQ(src, dest);

                for(ssize_t i = 0; i < recv_size; ++i)
                    WVASSERTEQ(recv_buf[i], send_buf[i]);
                break;
            }
        }
    }
}

void tudp() {
    // wait for UDP socket just once
    Semaphore::attach("net-udp").down();

    RUN_TEST(basics);
    RUN_TEST(connect);
    RUN_TEST(data);
}
