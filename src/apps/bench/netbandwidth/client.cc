/*
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

#include <base/DTU.h>
#include <base/Env.h>
#include <base/util/Time.h>

#include <m3/com/Semaphore.h>
#include <m3/session/NetworkManager.h>
#include <m3/stream/Standard.h>
#include <m3/Test.h>

using namespace m3;

int main() {
    NetworkManager net("net0");

    Socket *socket = net.create(Socket::SOCK_DGRAM);

    // wait for server
    Semaphore::attach("net").down();

    socket->blocking(true);
    socket->connect(IpAddr(192, 168, 112, 1), 1337);

    constexpr size_t packet_size = 1024;
    union {
        uint8_t raw[packet_size];
        cycles_t time;
    } request;

    union {
        uint8_t raw[packet_size];
        cycles_t time;
    } response;

    size_t warmup = 5;
    size_t packets_to_send = 105;
    size_t packets_to_receive = 100;
    size_t burst_size = 2;
    cycles_t timeout = 100000000;

    size_t packet_sent_count = 0;
    size_t packet_received_count = 0;
    size_t received_bytes = 0;

    cout << "Warmup...\n";
    while(warmup--) {
        socket->send(request.raw, 8);
        socket->recv(response.raw, 8);
    }
    cout << "Warmup done.\n";

    socket->blocking(false);
    cout << "Benchmark...\n";
    cycles_t start = Time::start(0);
    cycles_t last_received = start;
    size_t failures = 0;
    while(true) {
        // Wait for wakeup (message or credits received)
        if(failures >= 10) {
            failures = 0;
            DTUIf::sleep();
        }

        size_t send_count = burst_size;
        while(send_count-- && packet_sent_count < packets_to_send) {
            if(socket->send(request.raw, packet_size) > 0) {
                packet_sent_count++;
                failures = 0;
            } else {
                failures++;
                break;
            }
        }

        size_t receive_count = burst_size;
        while(receive_count--) {
            ssize_t recv_len = socket->recv(response.raw, packet_size);
            if(recv_len > 0) {
                received_bytes += static_cast<size_t>(recv_len);
                packet_received_count++;
                last_received = Time::start(0);
                failures = 0;
            } else {
               failures++;
               break;
            }
        }

        if(packet_received_count >= packets_to_receive)
            break;
        if(packet_sent_count == packets_to_send && Time::start(0) - last_received > timeout)
            break;
    }
    cout << "Benchmark done.\n";

    cout << "Sent packets: " << packet_sent_count << "\n";
    cout << "Received packets: " << packet_received_count << "\n";
    cout << "Received bytes: " << received_bytes << "\n";
    size_t duration = last_received - start;
    cout << "Duration: " << duration << "\n";
    float bps = static_cast<float>(received_bytes) / (duration / 3e9f);
    WVPERF("network bandwidth", bps << " bytes / s\n");

    delete socket;

    return 0;
}
