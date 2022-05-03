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

#include <base/CmdArgs.h>
#include <base/stream/IStringStream.h>
#include <base/time/Profile.h>
#include <base/util/Math.h>

#include <m3/Syscalls.h>
#include <m3/Test.h>
#include <m3/com/MemGate.h>
#include <m3/net/TcpSocket.h>
#include <m3/net/UdpSocket.h>
#include <m3/session/NetworkManager.h>
#include <m3/stream/Standard.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "encoder.h"
#include "handler.h"

using namespace m3;

constexpr size_t MAX_FILE_SIZE = 1024 * 1024;

static size_t recv_audio(void *dst, ClientSession &sess) {
    size_t size;
    KIF::CapRngDesc caps;
    KIF::ExchangeArgs args;
    ExchangeOStream os(args);
    os << /* RECV */ 0;
    args.bytes = os.total();
    caps = sess.obtain(1, &args);
    ExchangeIStream is(args);
    is >> size;

    MemGate audio = MemGate::bind(caps.start());
    audio.read(dst, size, 0);
    return size;
}

static TimeDuration forward_audio(ClientSession &vamic, OpHandler *hdl, void *mem, void *out,
                                  bool compute) {
    auto start = TimeInstant::now();

    size_t size = recv_audio(mem, vamic);

    m3::cout << "Encoding " << size << " bytes WAV\n";
    size_t res;
    if(compute)
        res = encode((const uint8_t *)mem, size, out, 1024 * 1024);
    else {
        res = 40 * 1024;
        memset(out, 0, res);
    }
    m3::cout << "Produced " << res << " bytes of FLAC\n";

    hdl->send(out, res);

    auto end = TimeInstant::now();
    m3::cout << "Iteration: " << end.duration_since(start) << "\n";
    return end.duration_since(start);
}

static void usage(const char *name) {
    fprintf(stderr, "Usage: %s [-r <repeats>] [-w <warmup>] [-c] (udp|tcp) <ip> <port>\n", name);
    fprintf(stderr, "  -r <repeats>: the number of runs\n");
    fprintf(stderr, "  -w <warmup>: the number of warmup runs\n");
    fprintf(stderr, "  -p: just pretend to use FLAC\n");
    exit(1);
}

int main(int argc, char **argv) {
    int warmup = 2;
    int repeats = 8;
    bool compute = true;

    int opt;
    while((opt = CmdArgs::get(argc, argv, "r:w:p")) != -1) {
        switch(opt) {
            case 'r': repeats = IStringStream::read_from<int>(CmdArgs::arg); break;
            case 'w': warmup = IStringStream::read_from<int>(CmdArgs::arg); break;
            case 'p': compute = false; break;
            default: usage(argv[0]);
        }
    }
    if(CmdArgs::ind + 3 != argc)
        usage(argv[0]);

    char *proto = argv[CmdArgs::ind + 0];
    IpAddr ip = IStringStream::read_from<IpAddr>(argv[CmdArgs::ind + 1]);
    port_t port = IStringStream::read_from<port_t>(argv[CmdArgs::ind + 2]);

    NetworkManager net("net");

    ClientSession vamic("vamic");

    OpHandler *hdl;
    if(strcmp(proto, "udp") == 0)
        hdl = new UDPOpHandler(net, ip, port);
    else
        hdl = new TCPOpHandler(net, ip, port);

    void *mem = malloc(MAX_FILE_SIZE);
    void *out = malloc(MAX_FILE_SIZE);

    for(int i = 0; i < warmup; ++i)
        forward_audio(vamic, hdl, mem, out, compute);

    Syscalls::reset_stats();
    auto wall_start = TimeInstant::now();

    Results<TimeDuration> res(static_cast<size_t>(repeats));
    for(int i = 0; i < repeats; ++i)
        res.push(forward_audio(vamic, hdl, mem, out, compute));
    WVPERF("VoiceAssistant with " << proto, res);

    free(out);
    free(mem);

    auto wall_stop = TimeInstant::now();
    m3::cout << "Total Time: " << wall_stop.duration_since(wall_start) << "\n";
    m3::cout << "\033[1;32mAll tests successful!\033[0;m\n";

    Syscalls::reset_stats();
    delete hdl;
    return 0;
}
