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

#include <m3/com/Semaphore.h>
#include <m3/net/UdpSocket.h>
#include <m3/session/NetworkManager.h>
#include <m3/stream/Standard.h>
#include <m3/vfs/FileRef.h>
#include <m3/vfs/VFS.h>

#include <base/CPU.h>
#include <base/TCU.h>

#include <endian.h>

#include "leveldb/db.h"
#include "leveldb/write_batch.h"

#include "ops.h"

#if defined(__kachel__)
#   define SYSC_RECEIVE     0xFFFF
#   define SYSC_SEND        0xFFFE
extern "C" void __m3_sysc_trace(bool enable, size_t max);
extern "C" void __m3_sysc_trace_start(long n);
extern "C" void __m3_sysc_trace_stop();
extern "C" uint64_t __m3_sysc_systime();
#endif

using namespace m3;

constexpr size_t MAX_FILE_SIZE = 4 * 1024 * 1024;

static uint64_t read_u64(const uint8_t *bytes) {
    uint64_t res = 0;
#if __BIG_ENDIAN
    for(size_t i = 0; i < 8; ++i)
        res |= static_cast<uint64_t>(bytes[i]) << (56 - i * 8);
#else
    for(size_t i = 0; i < 8; ++i)
        res |= static_cast<uint64_t>(bytes[i]) << (i * 8);
#endif
    return res;
}

static size_t from_bytes(uint8_t *package_buffer, Package *pkg) {
    pkg->op = package_buffer[0];
    pkg->table = package_buffer[1];
    pkg->num_kvs = package_buffer[2];
    pkg->key = read_u64(package_buffer + 3);
    pkg->scan_length = read_u64(package_buffer + 11);

    size_t pos = 19;
    for(size_t i = 0; i < pkg->num_kvs; ++i) {
        // check that the length is within the parameters
        size_t key_len = package_buffer[pos];
        size_t val_len = package_buffer[pos + 1];
        pos += 2;

        std::string key((const char*)package_buffer + pos, key_len);
        pos += key_len;

        std::string val((const char*)package_buffer + pos, val_len);
        pos += val_len;
        pkg->kv_pairs.push_back(std::make_pair(key, val));
    }

    return pos;
}

static uint8_t *wl_buffer;
static size_t wl_pos;
static size_t wl_size;

static uint32_t wl_read4b() {
    union {
        uint32_t word;
        uint8_t bytes[4];
    };
    memcpy(bytes, wl_buffer + wl_pos, sizeof(bytes));
    wl_pos += 4;
    return be32toh(word);
}

int main(int argc, char** argv) {
    if(argc != 6) {
        cerr << "Usage: " << argv[0] << " <ip> <port> <workload> <db> <repeats>\n";
        return 1;
    }

    IpAddr ip = IStringStream::read_from<IpAddr>(argv[1]);
    port_t port = IStringStream::read_from<port_t>(argv[2]);
    const char *workload = argv[3];
    const char *file = argv[4];
    int repeats = IStringStream::read_from<int>(argv[5]);

    VFS::mount("/", "m3fs", "m3fs");

    NetworkManager net("net0");

    auto socket = UdpSocket::create(net, DgramSocketArgs().send_buffer(4, 16 * 1024)
                                                          .recv_buffer(64, 512 * 1024));
    socket->bind(2000);

    wl_buffer = new uint8_t[MAX_FILE_SIZE];
    wl_pos = 0;
    wl_size = 0;
    {
        FileRef wl_file(workload, FILE_R);
        size_t len;
        while((len = wl_file->read(wl_buffer + wl_size, MAX_FILE_SIZE - wl_size)) > 0)
            wl_size += len;
    }

    UNUSED uint64_t total_preins = static_cast<uint64_t>(wl_read4b());
    uint64_t total_ops = static_cast<uint64_t>(wl_read4b());

    uint64_t opcounter = 0;

    cout << "Starting Benchmark:\n";
    Executor *exec = Executor::create(file);

    for(int i = 0; i < repeats; ++i) {
#if defined(__kachel__)
        __m3_sysc_trace(true, 32768);
#endif
        exec->reset_stats();
        opcounter = 0;
        wl_pos = 4 * 2;

        uint64_t start = m3::TCU::get().nanotime();

        while(opcounter < total_ops) {
            Package pkg;
            size_t package_size = from_bytes(wl_buffer + wl_pos, &pkg);
            wl_pos += package_size;

#if defined(__kachel__)
            __m3_sysc_trace_start(SYSC_SEND);
#endif
            socket->send_to(wl_buffer, package_size, Endpoint(ip, port));
#if defined(__kachel__)
            __m3_sysc_trace_stop();
#endif

            if((opcounter % 100) == 0)
                cout << "Op=" << pkg.op << " @ " << opcounter << "\n";

            exec->execute(pkg);
            opcounter += 1;
        }

        uint64_t end = m3::TCU::get().nanotime();
#if defined(__kachel__)
        cout << "Systemtime: " << (__m3_sysc_systime() / 1000) << " us\n";
#endif
        cout << "Totaltime: " << ((end - start) / 1000) << " us\n";

        cout << "Server Side:\n";
        exec->print_stats(opcounter);
    }

    // TODO hack to circumvent the missing credit problem during destruction
    socket.forget();

// #if defined(__kachel__)
//     __m3_sysc_trace(false, 0);
// #endif

    return 0;
}
