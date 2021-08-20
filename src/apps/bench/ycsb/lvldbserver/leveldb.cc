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

#include <iostream>
#include <sstream>
#include <string>

#include <base/stream/IStringStream.h>
#include <base/util/Profile.h>

#include <m3/session/NetworkManager.h>
#include <m3/stream/Standard.h>
#include <m3/vfs/VFS.h>
#include <m3/Test.h>

#include "leveldb/db.h"
#include "leveldb/write_batch.h"

#include "ops.h"
#include "handler.h"

using namespace m3;

int main(int argc, char** argv) {
    if(argc != 5 && argc != 7) {
        cerr << "Usage: " << argv[0] << " <db> <repeats> tcp <port>\n";
        cerr << "Usage: " << argv[0] << " <db> <repeats> udp <ip> <port> <workload>\n";
        return 1;
    }

    VFS::mount("/", "m3fs", "m3fs");

    NetworkManager net("net");

    const char *file = argv[1];
    int repeats = IStringStream::read_from<int>(argv[2]);

    Executor *exec = Executor::create(file);

    OpHandler *hdl;
    if(strcmp(argv[3], "tcp") == 0) {
        port_t port = IStringStream::read_from<port_t>(argv[4]);
        hdl = new TCPOpHandler(net, port);
    }
    else {
        IpAddr ip = IStringStream::read_from<IpAddr>(argv[4]);
        port_t port = IStringStream::read_from<port_t>(argv[5]);
        const char *workload = argv[6];
        hdl = new UDPOpHandler(net, workload, ip, port);
    }

    cout << "Starting Benchmark:\n";

    Results<MicroResult> res(static_cast<size_t>(repeats));
    for(int i = 0; i < repeats; ++i) {
        uint64_t opcounter = 0;

        __m3_sysc_trace(true, 32768);
        exec->reset_stats();
        hdl->reset();

        uint64_t start = m3::TCU::get().nanotime();

        bool run = true;
        while(run) {
            Package pkg;
            switch(hdl->receive(pkg)) {
                case OpHandler::STOP:
                    run = false;
                    continue;
                case OpHandler::INCOMPLETE:
                    continue;
                case OpHandler::READY:
                    break;
            }

            if((opcounter % 100) == 0)
                cout << "Op=" << pkg.op << " @ " << opcounter << "\n";

            size_t res_bytes = exec->execute(pkg);

            if(!hdl->respond(res_bytes))
                break;

            opcounter += 1;
        }

        uint64_t end = m3::TCU::get().nanotime();
        cout << "Systemtime: " << (__m3_sysc_systime() / 1000) << " us\n";
        cout << "Totaltime: " << ((end - start) / 1000) << " us\n";

        cout << "Server Side:\n";
        exec->print_stats(opcounter);
        res.push(end - start);
    }

    WVPERF("YCSB with " << argv[3], res);

    delete hdl;

    return 0;
}
