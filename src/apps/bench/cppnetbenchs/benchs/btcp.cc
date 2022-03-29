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
#include <base/time/Profile.h>
#include <base/Panic.h>

#include <m3/com/Semaphore.h>
#include <m3/net/TcpSocket.h>
#include <m3/session/NetworkManager.h>
#include <m3/stream/Standard.h>
#include <m3/Test.h>

#include "../cppnetbenchs.h"

using namespace m3;

NOINLINE static void latency() {
    NetworkManager net("net");

    auto socket = TcpSocket::create(net);

    // wait for server socket to be ready
    Semaphore::attach("net-tcp").down();

    socket->connect(Endpoint(IpAddr(192, 168, 112, 1), 1338));

    const size_t samples = 15;

    uint8_t buffer[1024];

    size_t warmup = 5;
    while(warmup--) {
        socket->send(buffer, 8);
        socket->recv(buffer, 8);
    }

    const size_t packet_size[] = {8, 16, 32, 64, 128, 256, 512, 1024};

    for(auto pkt_size : packet_size) {
        Results<TimeDuration> res(samples);

        while(res.runs() < samples) {
            auto start = TimeInstant::now();

            socket->send(buffer, pkt_size);
            size_t received = 0;
            while(received < pkt_size) {
                ssize_t res = socket->recv(buffer, pkt_size);
                if(res == -1)
                    exitmsg("Got empty package!");
                received += static_cast<size_t>(res);
            }

            auto duration = TimeInstant::now().duration_since(start);
            cout << "RTT (" << pkt_size << "b): " << duration.as_micros() << " us\n";
            res.push(duration);
        }

        WVPERF("network latency (" << pkt_size << "b)", MilliFloatResultRef<TimeDuration>(res));
    }

    socket->close();
}

NOINLINE static void bandwidth() {
    const size_t PACKETS_TO_SEND = 105;
    const size_t BURST_SIZE = 2;
    const TimeDuration TIMEOUT = TimeDuration::from_secs(1);

    NetworkManager net("net");

    auto socket = TcpSocket::create(net, StreamSocketArgs().send_buffer(64 * 1024)
                                                             .recv_buffer(256 * 1024));

    // wait for server socket to be ready
    Semaphore::attach("net-tcp").down();

    socket->connect(Endpoint(IpAddr(192, 168, 112, 1), 1338));

    constexpr size_t packet_size = 1024;

    uint8_t buffer[1024];

    for(int i = 0; i < 10; ++i) {
        socket->send(buffer, 8);
        socket->recv(buffer, sizeof(buffer));
    }

    socket->set_blocking(false);

    auto start                  = TimeInstant::now();
    auto last_received          = start;
    size_t sent_count           = 0;
    size_t received_count       = 0;
    size_t received_bytes       = 0;
    size_t failures             = 0;

    while(true) {
        // Wait for wakeup (message or credits received)
        if(failures >= 10) {
            failures = 0;
            if(sent_count >= PACKETS_TO_SEND) {
                auto waited = TimeInstant::now().duration_since(last_received);
                if(waited > TIMEOUT)
                    break;
                // we are not interested in output anymore
                net.wait_for(TIMEOUT - waited, NetworkManager::INPUT);
            }
            else
                net.wait();
        }

        for(size_t i = 0; i < BURST_SIZE; ++i) {
            if(sent_count >= PACKETS_TO_SEND)
                break;

            if(socket->send(buffer, packet_size) > 0) {
                sent_count++;
                failures = 0;
            }
            else {
                failures++;
                break;
            }
        }

        for(size_t i = 0; i < BURST_SIZE; ++i) {
            ssize_t pkt_size = socket->recv(buffer, sizeof(buffer));

            if(pkt_size != -1) {
                received_bytes += static_cast<size_t>(pkt_size);
                received_count++;
                last_received = TimeInstant::now();
                failures = 0;
            }
            else {
                failures++;
                break;
            }
        }

        if(received_bytes >= PACKETS_TO_SEND * sizeof(buffer))
            break;
    }

    cout << "Benchmark done.\n";

    cout << "Sent packets: " << sent_count << "\n";
    cout << "Received packets: " << received_count << "\n";
    cout << "Received bytes: " << received_bytes << "\n";
    auto duration = last_received.duration_since(start);
    cout << "Duration: " << duration << "\n";
    auto secs = static_cast<float>(duration.as_nanos()) / 1000000000.f;
    float mbps = (static_cast<float>(received_bytes) / secs) / (1024 * 1024);
    WVPERF("TCP bandwidth", mbps << " MiB/s (+/- 0 with 1 runs)\n");

    socket->set_blocking(true);
    socket->close();
}

void btcp() {
    RUN_BENCH(latency);
    RUN_BENCH(bandwidth);
}
