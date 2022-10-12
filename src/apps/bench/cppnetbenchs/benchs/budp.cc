/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2021 Nils Asmussen, Barkhausen Institut
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
#include <base/Panic.h>
#include <base/time/Profile.h>

#include <m3/Test.h>
#include <m3/com/Semaphore.h>
#include <m3/net/UdpSocket.h>
#include <m3/session/NetworkManager.h>
#include <m3/stream/Standard.h>
#include <m3/vfs/Waiter.h>

#include "../cppnetbenchs.h"

using namespace m3;

static Option<size_t> send_recv(FileWaiter &waiter, FileRef<UdpSocket> &socket,
                                const Endpoint &dest, const uint8_t *send_buf, size_t sbuf_size,
                                TimeDuration timeout, uint8_t *recv_buf, size_t rbuf_size) {
    socket->send_to(send_buf, sbuf_size, dest);

    waiter.wait_for(timeout);

    if(socket->has_data())
        return socket->recv(recv_buf, rbuf_size);
    return None;
}

NOINLINE static void latency() {
    const TimeDuration TIMEOUT = TimeDuration::from_secs(1);

    NetworkManager net("net");

    uint8_t request[1024];
    uint8_t response[1024];

    auto socket = UdpSocket::create(net);

    const size_t samples = 15;
    Endpoint src;
    Endpoint dest = Endpoint(IpAddr(192, 168, 112, 1), 1337);

    FileWaiter waiter;
    waiter.add(socket->fd(), File::INPUT);

    // do one initial send-receive with a higher timeout than the smoltcp-internal timeout to
    // workaround the high ARP-request delay with the loopback device.
    send_recv(waiter, socket, dest, request, 1, TimeDuration::from_secs(6), response,
              sizeof(response));

    size_t warmup = 5;
    while(warmup--)
        send_recv(waiter, socket, dest, request, 8, TIMEOUT, response, sizeof(response));

    const size_t packet_size[] = {8, 16, 32, 64, 128, 256, 512, 1024};

    for(auto pkt_size : packet_size) {
        Results<TimeDuration> res(samples);

        while(res.runs() < samples) {
            auto start = TimeInstant::now();

            size_t recv_len = send_recv(waiter, socket, dest, request, pkt_size, TIMEOUT, response,
                                        sizeof(response))
                                  .unwrap();

            auto stop = TimeInstant::now();

            WVASSERTEQ(recv_len, pkt_size);

            auto duration = stop.duration_since(start);
            println("RTT ({}b): {} us"_cf, pkt_size, duration.as_micros());
            res.push(duration);
        }

        auto name = OStringStream();
        format_to(name, "network latency ({}b)"_cf, pkt_size);
        WVPERF(name.str(), MilliFloatResultRef<TimeDuration>(res));
    }
}

NOINLINE static void bandwidth() {
    NetworkManager net("net");

    auto socket = UdpSocket::create(
        net, DgramSocketArgs().send_buffer(8, 64 * 1024).recv_buffer(32, 256 * 1024));

    constexpr size_t packet_size = 1024;

    uint8_t request[1024];
    uint8_t response[1024];

    Endpoint dest = Endpoint(IpAddr(192, 168, 112, 1), 1337);

    size_t warmup = 5;
    size_t packets_to_send = 105;
    size_t packets_to_receive = 100;
    size_t burst_size = 2;
    TimeDuration timeout = TimeDuration::from_secs(1);

    size_t packet_sent_count = 0;
    size_t packet_received_count = 0;
    size_t received_bytes = 0;

    FileWaiter waiter;
    waiter.add(socket->fd(), File::INPUT | File::OUTPUT);

    while(warmup--)
        send_recv(waiter, socket, dest, request, 8, timeout, response, sizeof(response));

    socket->set_blocking(false);

    auto start = TimeInstant::now();
    auto last_received = start;
    size_t failures = 0;
    while(true) {
        // Wait for wakeup (message or credits received)
        if(failures >= 10) {
            failures = 0;
            auto waited = TimeInstant::now().duration_since(last_received);
            if(waited > timeout)
                break;
            if(packet_sent_count >= packets_to_send) {
                // we are not interested in output anymore
                waiter.remove(socket->fd());
                waiter.add(socket->fd(), File::INPUT);
            }
            waiter.wait_for(timeout - waited);
        }

        size_t send_count = burst_size;
        while(send_count-- && packet_sent_count < packets_to_send) {
            if(socket->send_to(request, packet_size, dest)) {
                packet_sent_count++;
                failures = 0;
            }
            else {
                failures++;
                break;
            }
        }

        size_t receive_count = burst_size;
        while(receive_count--) {
            if(auto pkt_size = socket->recv(response, sizeof(response))) {
                received_bytes += pkt_size.unwrap();
                packet_received_count++;
                last_received = TimeInstant::now();
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

    println("Benchmark done."_cf);

    println("Sent packets: {}"_cf, packet_sent_count);
    println("Received packets: {}"_cf, packet_received_count);
    println("Received bytes: {}"_cf, received_bytes);
    auto duration = last_received.duration_since(start);
    println("Duration: {}"_cf, duration);
    auto secs = static_cast<float>(duration.as_nanos()) / 1000000000.f;
    float mbps = (static_cast<float>(received_bytes) / secs) / (1024 * 1024);

    auto res = OStringStream();
    format_to(res, "{}  MiB/s (+/- 0 with 1 runs)\n"_cf, mbps);
    WVPERF("network bandwidth", res.str());
}

void budp() {
    // wait for UDP socket just once
    Semaphore::attach("net-udp").down();

    RUN_BENCH(latency);
    RUN_BENCH(bandwidth);
}
