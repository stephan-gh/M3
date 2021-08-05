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

#include <stdio.h>
#include <string.h>
#include <stdlib.h>

#include "encoder.h"

using namespace m3;

static void usage(const char *name) {
    fprintf(stderr, "Usage: %s <ip> <port> <wav>\n", name);
    exit(1);
}

constexpr size_t MAX_FILE_SIZE = 1024 * 1024;

int main(int argc, char **argv) {
    if(argc != 4) {
        usage(argv[0]);
    }

    NetworkManager net("net");

    // IpAddr ip = IStringStream::read_from<IpAddr>(argv[1]);
    IpAddr ip(127, 0, 0, 1);
    port_t port = IStringStream::read_from<port_t>(argv[2]);

    auto socket = TcpSocket::create(net, StreamSocketArgs().send_buffer(512 * 1024));
    socket->connect(Endpoint(ip, port));

    for(int i = 0; i < 4; ++i) {
        FILE *f = fopen(argv[3], "r");
        if(!f) {
            fprintf(stderr, "fopen failed");
            return 1;
        }

        void *mem = malloc(MAX_FILE_SIZE);
        size_t size = fread(mem, 1, MAX_FILE_SIZE, f);
        fclose(f);

        printf("Encoding %zu bytes WAV\n", size);
        void *out = malloc(MAX_FILE_SIZE);
        size_t res = 55 * 1024;//encode((const uint8_t*)mem, size, out, 1024 * 1024);
        printf("Produced %zu bytes of FLAC\n", res);

        uint64_t length = res;
        if(socket->send(&length, sizeof(length)) != sizeof(length))
            fprintf(stderr, "send failed");

        size_t rem = res;
        char *out_bytes = static_cast<char*>(out);
        while(rem > 0) {
            size_t amount = Math::min(rem, static_cast<size_t>(1024));
            if(socket->send(out_bytes, amount) != static_cast<ssize_t>(amount))
                fprintf(stderr, "send failed");

            out_bytes += amount;
            rem -= amount;
        }

        free(out);
        free(mem);
    }

    socket->close();
    return 0;
}
