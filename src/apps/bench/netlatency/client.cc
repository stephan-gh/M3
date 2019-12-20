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

#include <base/util/Time.h>
#include <base/util/Profile.h>
#include <base/Env.h>

#include <m3/com/Semaphore.h>
#include <m3/session/NetworkManager.h>
#include <m3/stream/Standard.h>
#include <m3/Test.h>

using namespace m3;

int main() {
    NetworkManager net("net0");

    Socket *socket = net.create(Socket::SOCK_DGRAM);
    socket->blocking(true);

    // wait for server
    Semaphore::attach("net").down();

    socket->connect(IpAddr(192, 168, 112, 1), 1337);

    union {
        uint8_t raw[1024];
        cycles_t time;
    } request;

    union {
        uint8_t raw[1024];
        cycles_t time;
    } response;

    const size_t samples = 15;

    size_t warmup = 5;
    cout << "Warmup...\n";
    while(warmup--) {
        socket->send(request.raw, 8);
        socket->recv(response.raw, 8);
    }
    cout << "Warmup done.\n";

    cout << "Benchmark...\n";
    const size_t packet_size[] = {8, 16, 32, 64, 128, 256, 512, 1024};
    for(auto pkt_size : packet_size) {
        Results res(samples);
        while(res.runs() < samples) {
            cycles_t start = Time::start(0);

            request.time = start;
            ssize_t send_len = socket->send(request.raw, pkt_size);
            ssize_t recv_len = socket->recv(response.raw, pkt_size);

            cycles_t stop = Time::stop(0);

            if(static_cast<size_t>(send_len) != pkt_size)
                exitmsg("Send failed, expected " << pkt_size << ", got " << send_len);

            if(static_cast<size_t>(recv_len) != pkt_size || start != response.time)
                exitmsg("Receive failed, expected " << pkt_size << ", got " << recv_len);

            cout << "RTT (" << pkt_size << "b): " << stop - start << " cycles / " << (stop - start) / 3e6f << " ms (@3GHz) \n";

            res.push(stop - start);
        }

        OStringStream name;
        name << "network latency (" << pkt_size << "b)";
        WVPERF(name.str(), (res.avg() / 3e6f) << " ms (+/- " << (res.stddev() / 3e6f)
                                              << " with " << res.runs() << " runs)\n");
    }

    socket->close();
    delete socket;

    return 0;
}
