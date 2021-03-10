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
#include <base/util/Profile.h>
#include <base/util/Time.h>

#include <m3/Test.h>
#include <m3/com/Semaphore.h>
#include <m3/netrs/UdpSocket.h>
#include <m3/session/NetworkManagerRs.h>
#include <m3/stream/Standard.h>

using namespace m3;

union Package {
    uint8_t raw[1024];
    cycles_t time;
};

int main() {
    NetworkManagerRs net("net0");

    auto socket = UdpSocketRs::create(net);

    // wait for server
    Semaphore::attach("net").down();

    socket->bind(IpAddr(192, 168, 112, 2), 1337);

    union {
        uint8_t raw[1024];
        cycles_t time;
    } request;

    union {
        uint8_t raw[1024];
        cycles_t time;
    } response;

    const size_t samples = 15;
    IpAddr dest_addr     = IpAddr(192, 168, 112, 1);
    uint16_t dest_port   = 1337;
    IpAddr src_addr;
    uint16_t src_port;

    size_t warmup = 5;
    cout << "Warmup...\n";
    while(warmup--) {
        socket->sendto(request.raw, 8, dest_addr, dest_port);
        socket->recvfrom(response.raw, 8, &src_addr, &src_port);
    }
    cout << "Warmup done.\n";

    cout << "Benchmark...\n";
    const size_t packet_size[] = {8, 16, 32, 64, 128, 256, 512, 1024};
    for(auto pkt_size : packet_size) {
        Results res(samples);
        while(res.runs() < samples) {
            cycles_t start = Time::start(0);

            request.time = start;
            socket->sendto(request.raw, pkt_size, dest_addr, dest_port);
            // TODO smoltcp doesn't tell us how much was send...
            ssize_t send_len = static_cast<ssize_t>(pkt_size);
            ssize_t recv_len = socket->recvfrom(response.raw, pkt_size, &src_addr, &src_port);
            if(recv_len == -1) {
                exitmsg("Got empty package!");
            }
            cycles_t stop = Time::stop(0);

            if(static_cast<size_t>(send_len) != pkt_size)
                exitmsg("Send failed, expected " << pkt_size << ", got " << send_len);

            if(static_cast<size_t>(recv_len) != pkt_size || start != response.time) {
                cout << "Time should be " << start << " but was " << response.time
                     << "\n";
                exitmsg("Receive failed, expected " << pkt_size << ", got " << recv_len);
            }

            cout << "RTT (" << pkt_size << "b): " << stop - start << " cycles / " << (stop - start) / 3e6f
                 << " ms (@3GHz) \n";

            res.push(stop - start);
        }

        OStringStream name;
        name << "network latency (" << pkt_size << "b)";
        WVPERF(name.str(), (res.avg() / 3e6f) << " ms (+/- " << (res.stddev() / 3e6f) << " with "
                                              << res.runs() << " runs)\n");
    }

    return 0;
}
