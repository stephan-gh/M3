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
#include <m3/net/TcpSocket.h>
#include <m3/session/NetworkManager.h>
#include <m3/Test.h>

#include "../cppnettests.h"

using namespace m3;

static void basics() {
    NetworkManager net("net0");

    auto socket = TcpSocket::create(net);

    WVASSERTEQ(socket->state(), Socket::Closed);
    WVASSERTEQ(socket->local_endpoint(), Endpoint::unspecified());
    WVASSERTEQ(socket->remote_endpoint(), Endpoint::unspecified());

    Semaphore::attach("net-tcp").down();

    WVASSERTERR(Errors::NOT_CONNECTED, [&socket] {
        uint8_t dummy;
        socket->send(&dummy, sizeof(dummy));
    });

    socket->connect(Endpoint(IpAddr(192, 168, 112, 1), 1338));
    WVASSERTEQ(socket->state(), Socket::Connected);
    WVASSERTEQ(socket->local_endpoint().addr, IpAddr(192, 168, 112, 2));
    WVASSERTEQ(socket->remote_endpoint(), Endpoint(IpAddr(192, 168, 112, 1), 1338));

    uint8_t buf[32];
    WVASSERT(socket->send(buf, sizeof(buf)) != -1);
    WVASSERT(socket->recv(buf, sizeof(buf)) != -1);

    // connecting to the same remote endpoint is okay
    socket->connect(Endpoint(IpAddr(192, 168, 112, 1), 1338));
    // if anything differs, it's an error
    WVASSERTERR(Errors::IS_CONNECTED, [&socket] {
        socket->connect(Endpoint(IpAddr(192, 168, 112, 1), 1339));
    });
    WVASSERTERR(Errors::IS_CONNECTED, [&socket] {
        socket->connect(Endpoint(IpAddr(192, 168, 112, 2), 1338));
    });

    socket->abort();
    WVASSERTEQ(socket->state(), Socket::Closed);
    WVASSERTEQ(socket->local_endpoint(), Endpoint::unspecified());
    WVASSERTEQ(socket->remote_endpoint(), Endpoint::unspecified());
}

NOINLINE static void unreachable() {
    NetworkManager net("net0");

    auto socket = TcpSocket::create(net);

    WVASSERTERR(Errors::CONNECTION_FAILED, [&socket] {
        socket->connect(Endpoint(IpAddr(127, 0, 0, 1), 80));
    });
}

NOINLINE static void nonblocking_client() {
    NetworkManager net("net0");

    auto socket = TcpSocket::create(net);

    Semaphore::attach("net-tcp").down();

    socket->blocking(false);

    WVASSERT(!socket->connect(Endpoint(IpAddr(192, 168, 112, 1), 1338)));

    while(socket->state() != Socket::Connected) {
        WVASSERTEQ(socket->state(), Socket::Connecting);
        WVASSERTERR(Errors::ALREADY_IN_PROGRESS, [&socket] {
            socket->connect(Endpoint(IpAddr(192, 168, 112, 1), 1338));
        });
        net.wait(NetworkManager::INPUT);
    }

    uint8_t buf[32];

    for(int i = 0; i < 8; ++i) {
        while(socket->send(buf, sizeof(buf)) == -1)
            net.wait(NetworkManager::OUTPUT);
    }

    size_t total = 0;
    while(total < 8 * sizeof(buf)) {
        ssize_t res;
        while((res = socket->recv(buf, sizeof(buf))) == -1)
            net.wait(NetworkManager::INPUT);
        total += static_cast<size_t>(res);
    }
    WVASSERTEQ(total, 8 * sizeof(buf));

    while(socket->close() == Errors::WOULD_BLOCK)
        net.wait(NetworkManager::OUTPUT);

    while(socket->state() != Socket::Closed) {
        WVASSERTEQ(socket->state(), Socket::Closing);
        WVASSERTERR(Errors::ALREADY_IN_PROGRESS, [&socket] {
            socket->close();
        });
        net.wait(NetworkManager::INPUT);
    }
}

NOINLINE static void nonblocking_server() {
    auto pe = PE::get("clone|own");
    VPE vpe(pe, "tcp-server");

    auto sem = Semaphore::create(0);
    vpe.delegate_obj(sem.sel());

    vpe.data_sink() << sem.sel();

    vpe.run([] {
        capsel_t sem_sel;
        VPE::self().data_source() >> sem_sel;

        NetworkManager net("net1");

        auto socket = TcpSocket::create(net);

        socket->blocking(false);

        socket->listen(3000);
        WVASSERTEQ(socket->state(), Socket::Listening);

        auto sem = Semaphore::bind(sem_sel);
        sem.up();

        Endpoint remote_ep;
        WVASSERTEQ(socket->accept(&remote_ep), false);
        while(socket->state() == Socket::Connecting) {
            WVASSERTERR(Errors::ALREADY_IN_PROGRESS, [&socket, &remote_ep] {
                socket->accept(&remote_ep);
            });
            net.wait(NetworkManager::INPUT);
        }
        WVASSERT(socket->state() == Socket::Connected || socket->state() == Socket::RemoteClosed);

        WVASSERTEQ(socket->local_endpoint(), Endpoint(IpAddr(192, 168, 112, 1), 3000));
        WVASSERTEQ(socket->remote_endpoint().addr, IpAddr(192, 168, 112, 2));

        socket->blocking(true);
        socket->close();

        return 0;
    });

    NetworkManager net("net0");

    auto socket = TcpSocket::create(net);

    sem.down();

    socket->connect(Endpoint(IpAddr(192, 168, 112, 1), 3000));

    socket->close();

    WVASSERTEQ(vpe.wait(), 0);
}

NOINLINE static void open_close() {
    NetworkManager net("net0");

    auto socket = TcpSocket::create(net);

    Semaphore::attach("net-tcp").down();

    socket->connect(Endpoint(IpAddr(192, 168, 112, 1), 1338));
    socket->close();
    WVASSERTEQ(socket->state(), Socket::Closed);

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
    auto pe = PE::get("clone|own");
    VPE vpe(pe, "tcp-server");

    auto sem = Semaphore::create(0);
    vpe.delegate_obj(sem.sel());

    vpe.data_sink() << sem.sel();

    vpe.run([] {
        capsel_t sem_sel;
        VPE::self().data_source() >> sem_sel;

        NetworkManager net("net1");

        auto socket = TcpSocket::create(net);

        socket->listen(3000);
        WVASSERTEQ(socket->state(), Socket::Listening);

        auto sem = Semaphore::bind(sem_sel);
        sem.up();

        Endpoint remote_ep;
        socket->accept(&remote_ep);
        WVASSERTEQ(remote_ep.addr, IpAddr(192, 168, 112, 2));
        WVASSERTEQ(socket->state(), Socket::Connected);

        uint8_t buf[32];
        WVASSERTEQ(socket->recv(buf, sizeof(buf)), 32);
        WVASSERT(socket->send(buf, sizeof(buf)) != -1);

        socket->close();
        WVASSERTEQ(socket->state(), Socket::Closed);

        return 0;
    });

    NetworkManager net("net0");

    auto socket = TcpSocket::create(net);

    sem.down();

    socket->connect(Endpoint(IpAddr(192, 168, 112, 1), 3000));

    uint8_t buf[32];
    WVASSERT(socket->send(buf, sizeof(buf)) != -1);
    WVASSERTEQ(socket->recv(buf, sizeof(buf)), 32);

    // at some point, the socket should receive the closed event from the remote side
    while(socket->state() != Socket::RemoteClosed)
        net.wait(NetworkManager::INPUT);

    socket->close();

    WVASSERTEQ(vpe.wait(), 0);
}

NOINLINE static void data() {
    NetworkManager net("net0");

    auto socket = TcpSocket::create(net, StreamSocketArgs().send_buffer(2 * 1024));

    Semaphore::attach("net-tcp").down();

    socket->connect(Endpoint(IpAddr(192, 168, 112, 1), 1338));

    // disable 256 to workaround the bug in gem5's E1000 model
    size_t packet_sizes[] = {8, 16, 32, 64, 128, /*256,*/ 512, 934, 1024};
    for(auto pkt_size : packet_sizes) {
        uint8_t recv_buf[pkt_size * 8];
        uint8_t send_buf[pkt_size * 8];
        for(size_t i = 0; i < sizeof(send_buf); ++i)
            send_buf[i] = i;

        for(size_t i = 0; i < 8; ++i)
            WVASSERT(socket->send(send_buf + pkt_size * i, pkt_size) == static_cast<ssize_t>(pkt_size));

        uint8_t expected_byte = 0;
        size_t received = 0;
        while(received < pkt_size * 8) {
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
    RUN_TEST(nonblocking_client);
    RUN_TEST(nonblocking_server);
    RUN_TEST(open_close);
    RUN_TEST(receive_after_close);
    RUN_TEST(data);
}
