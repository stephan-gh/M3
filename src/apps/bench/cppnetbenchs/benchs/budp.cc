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
#include <base/util/Profile.h>
#include <base/Panic.h>

#include <m3/com/Semaphore.h>
#include <m3/netrs/UdpSocket.h>
#include <m3/session/NetworkManagerRs.h>
#include <m3/stream/Standard.h>
#include <m3/Test.h>

#include "../cppnetbenchs.h"

using namespace m3;

union {
    uint8_t raw[1024];
    cycles_t time;
} request;

union {
    uint8_t raw[1024];
    cycles_t time;
} response;

NOINLINE static void latency() {
    NetworkManagerRs net("net0");

    auto socket = UdpSocketRs::create(net);

    socket->bind(2000);

    const size_t samples = 15;
    IpAddr dest_addr     = IpAddr(192, 168, 112, 1);
    uint16_t dest_port   = 1337;
    IpAddr src_addr;
    uint16_t src_port;

    size_t warmup = 5;
    while(warmup--) {
        socket->send_to(request.raw, 8, dest_addr, dest_port);
        socket->recv_from(response.raw, 8, &src_addr, &src_port);
    }

    const size_t packet_size[] = {8, 16, 32, 64, 128, 256, 512, 1024};

    for(auto pkt_size : packet_size) {
        Results res(samples);

        while(res.runs() < samples) {
            cycles_t start = Time::start(0);

            request.time = start;
            ssize_t send_len = socket->send_to(request.raw, pkt_size, dest_addr, dest_port);
            ssize_t recv_len = socket->recv_from(response.raw, pkt_size, &src_addr, &src_port);
            if(recv_len == -1)
                exitmsg("Got empty package!");
            cycles_t stop = Time::stop(0);

            if(static_cast<size_t>(send_len) != pkt_size)
                exitmsg("Send failed, expected " << pkt_size << ", got " << send_len);

            if(static_cast<size_t>(recv_len) != pkt_size || start != response.time) {
                cout << "Time should be " << start << " but was " << response.time << "\n";
                exitmsg("Receive failed, expected " << pkt_size << ", got " << recv_len);
            }

            cout << "RTT (" << pkt_size << "b): "
                 << stop - start << " cycles / " << (stop - start) / 3e6f << " ms (@3GHz) \n";

            res.push(stop - start);
        }

        OStringStream name;
        name << "network latency (" << pkt_size << "b)";
        WVPERF(name.str(), (res.avg() / 3e6f) << " ms (+/- " << (res.stddev() / 3e6f) << " with "
                                              << res.runs() << " runs)\n");
    }
}

NOINLINE static void bandwidth() {
    NetworkManagerRs net("net0");

    auto socket = UdpSocketRs::create(net, DgramSocketArgs().send_buffer(8, 64 * 1024)
                                                            .recv_buffer(32, 256 * 1024));

    socket->bind(2001);

    constexpr size_t packet_size = 1024;
    IpAddr dest_addr             = IpAddr(192, 168, 112, 1);
    uint16_t dest_port           = 1337;

    IpAddr src_addr;
    uint16_t src_port;

    size_t warmup             = 5;
    size_t packets_to_send    = 105;
    size_t packets_to_receive = 100;
    size_t burst_size         = 2;
    uint64_t timeout          = 1000000000; // 1sec

    size_t packet_sent_count     = 0;
    size_t packet_received_count = 0;
    size_t received_bytes        = 0;

    while(warmup--) {
        socket->send_to(request.raw, 8, dest_addr, dest_port);
        socket->recv_from(response.raw, sizeof(response.raw), &src_addr, &src_port);
    }

    socket->blocking(false);

    uint64_t start         = TCU::get().nanotime();
    uint64_t last_received = start;
    size_t failures        = 0;
    while(true) {
        // Wait for wakeup (message or credits received)
        if(failures >= 10) {
            failures = 0;
            if(packet_sent_count >= packets_to_send) {
                auto waited = TCU::get().nanotime() - last_received;
                if(waited > timeout)
                    break;
                // we are not interested in output anymore
                net.wait_for(timeout - waited, NetworkManagerRs::INPUT);
            }
            else
                net.wait();
        }

        size_t send_count = burst_size;
        while(send_count-- && packet_sent_count < packets_to_send) {
            if(socket->send_to(request.raw, packet_size, dest_addr, dest_port) > 0) {
                packet_sent_count++;
                failures = 0;
            } else {
                failures++;
                break;
            }
        }

        size_t receive_count = burst_size;
        while(receive_count--) {
            ssize_t pkt_size = socket->recv_from(response.raw, sizeof(response.raw), &src_addr, &src_port);

            if(pkt_size != -1) {
                received_bytes += static_cast<size_t>(pkt_size);
                packet_received_count++;
                last_received = TCU::get().nanotime();
                failures = 0;
            }
            else {
                failures++;
                break;
            }
        }

        if(packet_received_count >= packets_to_receive)
            break;
    }

    cout << "Benchmark done.\n";

    cout << "Sent packets: " << packet_sent_count << "\n";
    cout << "Received packets: " << packet_received_count << "\n";
    cout << "Received bytes: " << received_bytes << "\n";
    uint64_t duration = last_received - start;
    cout << "Duration: " << duration << "\n";
    float mbps = (static_cast<float>(received_bytes) / (duration / 1e9f)) / (1024 * 1024);
    WVPERF("network bandwidth", mbps << " MiB/s (+/- 0 with 1 runs)\n");
}

void budp() {
    // wait for UDP socket just once
    Semaphore::attach("net-udp").down();

    RUN_BENCH(latency);
    RUN_BENCH(bandwidth);
}
