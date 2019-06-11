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
#include <base/Env.h>

#include <m3/session/NetworkManager.h>
#include <m3/stream/Standard.h>

using namespace m3;

int main() {
    NetworkManager net("net0");

    Socket *socket = net.create(Socket::SOCK_STREAM);
    if(!socket)
        exitmsg("Socket creation failed");

    socket->blocking(true);
    Errors::Code err = socket->connect(IpAddr(192, 168, 112, 1), 1337);
    if(err != Errors::NONE)
        exitmsg("Socket connect failed: " << Errors::to_string(err));

    cout << "Socket connected!\n";
    cout << "Sending...\n";
    MemGate mem(MemGate::create_global(8192, MemGate::RW));
    fd_t fd;
    err = net.as_file(socket->sd(), FILE_RW, mem, 4096, fd);
    if(err != Errors::NONE)
        exitmsg("as_file failed: " << Errors::to_string(err));

    Reference<File> file = VPE::self().fds()->get(fd);

    cout << "Accessing socket as file: " << fd << " (" << file.get() <<")...\n";

    constexpr size_t packet_size = 1024;
    union {
        uint8_t raw[packet_size];
        cycles_t time;
    } request;

    size_t packets_to_send = 505;
    size_t packet_sent_count = 0;

    cout << "Benchmark...\n";
    while(packet_sent_count < packets_to_send) {
        ssize_t result = file->write(request.raw, packet_size);
        if(static_cast<ssize_t>(packet_size) != result)
            exitmsg("Writing has failed: " << result);
        packet_sent_count++;
    }
    file->flush();

    cout << "Sent packets: " << packet_sent_count << "\n";

    delete socket;

    return 0;
}
