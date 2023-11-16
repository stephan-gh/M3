/*
 * Copyright (C) 2021-2022 Nils Asmussen, Barkhausen Institut
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
#include <m3/net/TcpSocket.h>
#include <m3/session/Network.h>
#include <m3/tiles/ChildActivity.h>
#include <m3/vfs/Waiter.h>

#include "../cppnettests.h"

using namespace m3;

static void basics() {
    Network net("net0");

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
    WVASSERT(socket->send(buf, sizeof(buf)).is_some());
    WVASSERT(socket->recv(buf, sizeof(buf)).is_some());

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
    Network net("net0");

    auto socket = TcpSocket::create(net);

    WVASSERTERR(Errors::CONNECTION_FAILED, [&socket] {
        socket->connect(Endpoint(IpAddr(127, 0, 0, 1), 80));
    });
}

NOINLINE static void nonblocking_client() {
    Network net("net0");

    auto socket = TcpSocket::create(net);

    Semaphore::attach("net-tcp").down();

    socket->set_blocking(false);

    WVASSERT(!socket->connect(Endpoint(IpAddr(192, 168, 112, 1), 1338)));

    FileWaiter in_waiter, out_waiter;
    in_waiter.add(socket->fd(), File::INPUT);
    out_waiter.add(socket->fd(), File::OUTPUT);

    while(socket->state() != Socket::Connected) {
        WVASSERTEQ(socket->state(), Socket::Connecting);
        WVASSERTERR(Errors::ALREADY_IN_PROGRESS, [&socket] {
            socket->connect(Endpoint(IpAddr(192, 168, 112, 1), 1338));
        });
        in_waiter.wait();
    }

    uint8_t buf[32];

    for(int i = 0; i < 8; ++i) {
        while(socket->send(buf, sizeof(buf)).is_none())
            out_waiter.wait();
    }

    size_t total = 0;
    while(total < 8 * sizeof(buf)) {
        Option<size_t> res = None;
        while((res = socket->recv(buf, sizeof(buf))).is_none())
            in_waiter.wait();
        total += res.unwrap();
    }
    WVASSERTEQ(total, 8 * sizeof(buf));

    while(socket->close() == Errors::WOULD_BLOCK)
        out_waiter.wait();

    while(socket->state() != Socket::Closed) {
        WVASSERTEQ(socket->state(), Socket::Closing);
        WVASSERTERR(Errors::ALREADY_IN_PROGRESS, [&socket] {
            socket->close();
        });
        in_waiter.wait();
    }
}

NOINLINE static void nonblocking_server() {
    auto tile = Tile::get("compat|own");
    ChildActivity act(tile, "tcp-server");

    auto sem = Semaphore::create(0);
    act.delegate_obj(sem.sel());

    act.data_sink() << sem.sel();

    act.run([] {
        capsel_t sem_sel;
        Activity::own().data_source() >> sem_sel;

        Network net("net1");

        auto socket = TcpSocket::create(net);

        socket->set_blocking(false);

        socket->listen(3000);
        WVASSERTEQ(socket->state(), Socket::Listening);

        auto sem = Semaphore::bind(sem_sel);
        sem.up();

        FileWaiter waiter;
        waiter.add(socket->fd(), File::INPUT);

        Endpoint remote_ep;
        WVASSERTEQ(socket->accept(&remote_ep), false);
        while(socket->state() == Socket::Connecting) {
            WVASSERTERR(Errors::ALREADY_IN_PROGRESS, [&socket, &remote_ep] {
                socket->accept(&remote_ep);
            });
            waiter.wait();
        }
        WVASSERT(socket->state() == Socket::Connected || socket->state() == Socket::RemoteClosed);

        WVASSERTEQ(socket->local_endpoint(), Endpoint(IpAddr(192, 168, 112, 1), 3000));
        // if the network stack receives *both* the connected message and the close message before
        // we get any event, we only receive the close message and thus are not connected and do not
        // know the remote EP.
        if(socket->state() == Socket::Connected)
            WVASSERTEQ(socket->remote_endpoint().addr, IpAddr(192, 168, 112, 2));

        socket->set_blocking(true);
        socket->close();

        return 0;
    });

    Network net("net0");

    auto socket = TcpSocket::create(net);

    sem.down();

    socket->connect(Endpoint(IpAddr(192, 168, 112, 1), 3000));

    socket->close();

    WVASSERTEQ(act.wait(), 0);
}

NOINLINE static void open_close() {
    Network net("net0");

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
    auto tile = Tile::get("compat|own");
    ChildActivity act(tile, "tcp-server");

    auto sem = Semaphore::create(0);
    act.delegate_obj(sem.sel());

    act.data_sink() << sem.sel();

    act.run([] {
        capsel_t sem_sel;
        Activity::own().data_source() >> sem_sel;

        Network net("net1");

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
        WVASSERTEQ(socket->recv(buf, sizeof(buf)).unwrap_or(0), 32U);
        WVASSERT(socket->send(buf, sizeof(buf)).is_some());

        socket->close();
        WVASSERTEQ(socket->state(), Socket::Closed);

        return 0;
    });

    Network net("net0");

    auto socket = TcpSocket::create(net);

    sem.down();

    socket->connect(Endpoint(IpAddr(192, 168, 112, 1), 3000));

    uint8_t buf[32];
    WVASSERT(socket->send(buf, sizeof(buf)).is_some());
    WVASSERTEQ(socket->recv(buf, sizeof(buf)).unwrap_or(0), 32U);

    FileWaiter waiter;
    waiter.add(socket->fd(), File::INPUT);

    // at some point, the socket should receive the closed event from the remote side
    while(socket->state() != Socket::RemoteClosed)
        waiter.wait();

    socket->close();

    WVASSERTEQ(act.wait(), 0);
}

NOINLINE static void data() {
    Network net("net0");

    auto socket = TcpSocket::create(net, StreamSocketArgs().send_buffer(2 * 1024));

    Semaphore::attach("net-tcp").down();

    socket->connect(Endpoint(IpAddr(192, 168, 112, 1), 1338));

    // disable 256 to workaround the bug in gem5's E1000 model
    size_t packet_sizes[] = {8, 16, 32, 64, 128, /*256,*/ 512, 934, 1024, 2048, 4096};
    for(auto pkt_size : packet_sizes) {
        std::unique_ptr<uint8_t[]> recv_buf(new uint8_t[pkt_size]);
        std::unique_ptr<uint8_t[]> send_buf(new uint8_t[pkt_size * 8]);
        for(size_t i = 0; i < pkt_size * 8; ++i)
            send_buf[i] = i;

        for(size_t i = 0; i < 8; ++i)
            WVASSERT(socket->send(send_buf.get() + pkt_size * i, pkt_size).unwrap() == pkt_size);

        uint8_t expected_byte = 0;
        size_t received = 0;
        while(received < pkt_size * 8) {
            size_t recv_size = socket->recv(recv_buf.get(), pkt_size).unwrap();
            for(size_t i = 0; i < recv_size; ++i) {
                WVASSERTEQ(recv_buf.get()[i], expected_byte);
                expected_byte++;
            }
            received += recv_size;
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
