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

#include <base/util/Profile.h>
#include <base/Env.h>

#include <m3/accel/StreamAccel.h>
#include <m3/com/Semaphore.h>
#include <m3/session/NetworkManager.h>
#include <m3/stream/Standard.h>

using namespace m3;

int main() {
    auto sem = Semaphore::attach("net");

    NetworkManager net("net1");
    String status;

    Socket * socket = net.create(Socket::SOCK_STREAM);
    if(!socket)
        exitmsg("Socket creation failed");

    socket->blocking(true);
    Errors::Code err = socket->bind(IpAddr(192, 168, 112, 1), 1337);
    if(err != Errors::NONE)
        exitmsg("Socket bind failed: " << Errors::to_string(err));

    socket->listen();

    // notify client
    sem.up();

    Socket * accepted_socket = 0;
    err = socket->accept(accepted_socket);
    if(err != Errors::NONE)
        exitmsg("Socket accept failed: " << Errors::to_string(err));

    cout << "Socket accepted!\n";

    // TODO somehow we need to choose a large size to prevent that we get stuck (on host)
    MemGate mem(MemGate::create_global(64 * 1024, MemGate::RW));
    fd_t fd;
    err = net.as_file(accepted_socket->sd(), FILE_RW, mem, 32 * 1024, fd);
    if(err != Errors::NONE)
        exitmsg("as_file failed: " << Errors::to_string(err));
    Reference<File> file = VPE::self().fds()->get(fd);

    constexpr size_t packet_size = 1024;
    union {
        uint8_t raw[packet_size];
        cycles_t time;
    } response;

    size_t packets_to_receive = 500;
    size_t bytes_to_receive = packets_to_receive * packet_size;

    size_t packet_received_count = 0;
    size_t received_bytes = 0;

    cout << "Benchmark...\n";
    cycles_t first_received = 0;
    cycles_t last_received = 0;
    while(received_bytes < bytes_to_receive) {
        ssize_t recv_len = file->read(response.raw, packet_size);
        if(recv_len <= 0) {
            errmsg("Reading has failed: " << recv_len);
            break;
        }
        received_bytes += static_cast<size_t>(recv_len);

        if(packet_received_count == 0)
            first_received = Time::start(0);
        last_received = Time::start(0);

        packet_received_count++;
    }
    cout << "Benchmark done.\n";

    cout << "Received packets: " << packet_received_count << "\n";
    cout << "Received bytes: " << received_bytes << "\n";
    size_t duration = last_received - first_received;
    cout << "Duration: " << duration << "\n";
    cout << "Rate: " << static_cast<float>(received_bytes) / duration << " bytes / cycle\n";
    cout << "Rate: " << static_cast<float>(received_bytes) / (duration / 3e9f) << " bytes / s\n";

    delete accepted_socket;
    delete socket;

    sem.up();

    return 0;
}
