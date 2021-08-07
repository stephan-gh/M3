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

#include <base/stream/IStringStream.h>
#include <base/util/Math.h>

#include <m3/session/NetworkManager.h>
#include <m3/net/TcpSocket.h>
#include <m3/net/UdpSocket.h>
#include <m3/stream/Standard.h>
#include <m3/Syscalls.h>

#include <stdio.h>
#include <string.h>
#include <stdlib.h>

#include "encoder.h"

using namespace m3;

static void usage(const char *name) {
    fprintf(stderr, "Usage: %s <ip> <port> <wav>\n", name);
    exit(1);
}

#define UDP 1

constexpr size_t MAX_FILE_SIZE = 1024 * 1024;
constexpr int REPEATS = 16;

int main(int argc, char **argv) {
    if(argc != 4) {
        usage(argv[0]);
    }

    IpAddr ip = IStringStream::read_from<IpAddr>(argv[1]);
    port_t port = IStringStream::read_from<port_t>(argv[2]);

    NetworkManager net("net");

#if UDP
    auto socket = UdpSocket::create(net, DgramSocketArgs().send_buffer(64, 128 * 1024));
    auto dest = Endpoint(ip, port);

    socket->bind(2000);
#else
    auto socket = TcpSocket::create(net, StreamSocketArgs().send_buffer(32 * 1024));

    m3::cout << "Connecting to " << ip << ":" << port << "...\n";
    socket->connect(Endpoint(ip, port));
    m3::cout << "Connection established\n";
#endif

    void *mem = malloc(MAX_FILE_SIZE);
    void *out = malloc(MAX_FILE_SIZE);

    Syscalls::reset_stats();
    uint64_t wall_start = TCU::get().nanotime();

    for(int i = 0; i < REPEATS; ++i) {
        uint64_t start = TCU::get().nanotime();

        FILE *f = fopen(argv[3], "r");
        if(!f) {
            fprintf(stderr, "fopen failed");
            return 1;
        }

        size_t size = fread(mem, 1, MAX_FILE_SIZE, f);
        fclose(f);

        m3::cout << "Encoding " << size << " bytes WAV\n";
        size_t res = encode((const uint8_t*)mem, size, out, 1024 * 1024);
        m3::cout << "Produced " << res << " bytes of FLAC\n";

#if UDP
        size_t rem = res;
        char *out_bytes = static_cast<char*>(out);
        while(rem > 0) {
            size_t amount = Math::min(rem, static_cast<size_t>(512));
            if(socket->send_to(out_bytes, amount, dest) != static_cast<ssize_t>(amount))
                m3::cerr << "send failed\n";

            out_bytes += amount;
            rem -= amount;
        }
#else
        uint64_t length = res;
        if(socket->send(&length, sizeof(length)) != sizeof(length))
            m3::cerr << "send failed\n";

        size_t rem = res;
        char *out_bytes = static_cast<char*>(out);
        while(rem > 0) {
            size_t amount = Math::min(rem, static_cast<size_t>(1024));
            if(socket->send(out_bytes, amount) != static_cast<ssize_t>(amount))
                m3::cerr << "send failed\n";

            out_bytes += amount;
            rem -= amount;
        }

        m3::cout << "waiting for ACK\n";

        char dummy;
        if(socket->recv(&dummy, sizeof(dummy)) != sizeof(dummy))
            m3::cerr << "receive failed\n";
#endif

        uint64_t end = TCU::get().nanotime();
        m3::cout << "Time: " << (end - start) << "\n";
    }

    free(out);
    free(mem);

#if !UDP
    socket->close();
#endif

    // TODO hack to circumvent the missing credit problem during destruction
    socket.forget();

    uint64_t wall_stop = TCU::get().nanotime();
    m3::cout << "Total Time: " << (wall_stop - wall_start) << "ns\n";

    return 0;
}
