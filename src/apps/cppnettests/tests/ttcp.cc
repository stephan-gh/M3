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
#include <m3/netrs/TcpSocket.h>
#include <m3/session/NetworkManagerRs.h>
#include <m3/Test.h>

#include "../cppnettests.h"

using namespace m3;

static void basics() {
    NetworkManagerRs net("net0");

    auto socket = TcpSocketRs::create(net);

    WVASSERTEQ(socket->state(), SocketRs::Closed);

    Semaphore::attach("net-tcp").down();

    WVASSERTERR(Errors::NOT_CONNECTED, [&socket] {
        uint8_t dummy;
        socket->send(&dummy, sizeof(dummy));
    });

    socket->connect(IpAddr(192, 168, 112, 1), 1338);
    WVASSERTEQ(socket->state(), SocketRs::Connected);

    uint8_t buf[32];
    WVASSERT(socket->send(buf, sizeof(buf)) != -1);
    WVASSERT(socket->recv(buf, sizeof(buf)) != -1);

    // connecting to the same remote endpoint is okay
    socket->connect(IpAddr(192, 168, 112, 1), 1338);
    // if anything differs, it's an error
    WVASSERTERR(Errors::IS_CONNECTED, [&socket] {
        socket->connect(IpAddr(192, 168, 112, 1), 1339);
    });
    WVASSERTERR(Errors::IS_CONNECTED, [&socket] {
        socket->connect(IpAddr(192, 168, 112, 2), 1338);
    });

    socket->abort();
    WVASSERTEQ(socket->state(), SocketRs::Closed);
}

NOINLINE static void unreachable() {
    NetworkManagerRs net("net0");

    auto socket = TcpSocketRs::create(net);

    WVASSERTERR(Errors::CONNECTION_FAILED, [&socket] {
        socket->connect(IpAddr(127, 0, 0, 1), 80);
    });
}

NOINLINE static void open_close() {
    NetworkManagerRs net("net0");

    auto socket = TcpSocketRs::create(net);

    Semaphore::attach("net-tcp").down();

    socket->connect(IpAddr(192, 168, 112, 1), 1338);
    socket->close();
    WVASSERTEQ(socket->state(), SocketRs::Closed);

    WVASSERTERR(Errors::NOT_CONNECTED, [&socket] {
        uint8_t dummy;
        socket->send(&dummy, sizeof(dummy));
    });
    WVASSERTERR(Errors::NOT_CONNECTED, [&socket] {
        uint8_t dummy;
        socket->recv(&dummy, sizeof(dummy));
    });
}

NOINLINE static void receive_after_close() {
    auto pe = PE::alloc(VPE::self().pe_desc());
    VPE vpe(pe, "tcp-server");

    auto sem = Semaphore::create(0);
    auto sem_sel = sem.sel();
    vpe.delegate_obj(sem_sel);

    vpe.run([&sem] {
        NetworkManagerRs net("net1");

        auto socket = TcpSocketRs::create(net);

        socket->listen(3000);
        WVASSERTEQ(socket->state(), SocketRs::Listening);

        sem.up();

        IpAddr remote_addr;
        uint16_t remote_port;
        socket->accept(&remote_addr, &remote_port);
        WVASSERTEQ(remote_addr.addr(), IpAddr(192, 168, 112, 2).addr());
        WVASSERTEQ(socket->state(), SocketRs::Connected);

        uint8_t buf[32];
        WVASSERTEQ(socket->recv(buf, sizeof(buf)), 32);
        WVASSERT(socket->send(buf, sizeof(buf)) != -1);

        socket->close();
        WVASSERTEQ(socket->state(), SocketRs::Closed);

        return 0;
    });

    NetworkManagerRs net("net0");

    auto socket = TcpSocketRs::create(net);

    sem.down();

    socket->connect(IpAddr(192, 168, 112, 1), 3000);

    uint8_t buf[32];
    WVASSERT(socket->send(buf, sizeof(buf)) != -1);
    WVASSERTEQ(socket->recv(buf, sizeof(buf)), 32);

    // at some point, the socket should receive the closed event from the remote side
    while(socket->state() != SocketRs::RemoteClosed)
        net.wait(NetworkManagerRs::INPUT);

    socket->close();

    WVASSERTEQ(vpe.wait(), 0);
}

NOINLINE static void data() {
    NetworkManagerRs net("net0");

    auto socket = TcpSocketRs::create(net);

    Semaphore::attach("net-tcp").down();

    socket->connect(IpAddr(192, 168, 112, 1), 1338);

    uint8_t send_buf[1024];
    for(int i = 0; i < 1024; ++i)
        send_buf[i] = i;

    uint8_t recv_buf[1024];

    size_t packet_sizes[] = {8, 16, 32, 64, 128, 256, 512, 1024};

    for(auto pkt_size : packet_sizes) {
        WVASSERT(socket->send(send_buf, pkt_size) != -1);

        size_t received = 0;
        uint8_t expected_byte = 0;
        while(received < pkt_size) {
            ssize_t recv_size = socket->recv(recv_buf, sizeof(recv_buf));
            WVASSERT(recv_size != -1);

            for(ssize_t i = 0; i < recv_size; ++i) {
                WVASSERTEQ(recv_buf[i], expected_byte);
                expected_byte++;
            }
            received += static_cast<size_t>(recv_size);
        }
    }
}

void ttcp() {
    RUN_TEST(basics);
    RUN_TEST(unreachable);
    RUN_TEST(open_close);
    RUN_TEST(receive_after_close);
    RUN_TEST(data);
}
