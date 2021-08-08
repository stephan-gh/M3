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
#include <m3/net/TcpSocket.h>
#include <m3/session/NetworkManager.h>
#include <m3/stream/Standard.h>
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
#endif

using namespace m3;

static uint8_t package_buffer[8 * 1024];

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

static bool from_bytes(uint8_t *package_buffer, size_t package_size, Package *pkg) {
    pkg->op = package_buffer[0];
    pkg->table = package_buffer[1];
    pkg->num_kvs = package_buffer[2];
    pkg->key = read_u64(package_buffer + 3);
    pkg->scan_length = read_u64(package_buffer + 11);

    size_t pos = 19;
    for(size_t i = 0; i < pkg->num_kvs; ++i) {
        if(pos + 2 > package_size)
            return false;

        // check that the length is within the parameters
        size_t key_len = package_buffer[pos];
        size_t val_len = package_buffer[pos + 1];
        pos += 2;
        if(pos + key_len + val_len > package_size)
            return false;

        std::string key((const char*)package_buffer + pos, key_len);
        pos += key_len;

        std::string val((const char*)package_buffer + pos, val_len);
        pos += val_len;
        pkg->kv_pairs.push_back(std::make_pair(key, val));
    }

    return true;
}

int main(int argc, char** argv) {
    if(argc != 4) {
        cerr << "Usage: " << argv[0] << " <ip> <port> <file>\n";
        return 1;
    }

    VFS::mount("/", "m3fs", "m3fs");

    NetworkManager net("net0");

    auto socket = TcpSocket::create(net, StreamSocketArgs().send_buffer(64 * 1024)
                                                           .recv_buffer(256 * 1024));

    IpAddr ip = IStringStream::read_from<IpAddr>(argv[1]);
    port_t port = IStringStream::read_from<port_t>(argv[2]);
    const char *file = argv[3];

    for(int i = 0; i < 3; ++i) {
        cout << "Connecting to " << ip << ":" << port << "...\n";
        try {
            socket->connect(Endpoint(ip, port));
            break;
        }
        catch(...) {
            cout << "Connection failed\n";
        }
    }

    uint64_t recv_timing = 0;
    uint64_t op_timing = 0;
    uint64_t opcounter = 0;

    cout << "Starting Benchmark:\n";
    Executor *exec = Executor::create(file);

    bool run = true;
    while(run) {
#if defined(__kachel__)
        __m3_sysc_trace(true, 32768);
#endif
        exec->reset_stats();
        recv_timing = 0;
        op_timing = 0;
        opcounter = 0;

        cycles_t start = m3::CPU::elapsed_cycles();

        while(1) {
            // Receiving a package is a two step process. First we receive a u32, which carries the
            // number of bytes the following package is big. We then wait until we received all those
            // bytes. After that the package is parsed and send to the database.
            uint64_t recv_start = m3::CPU::elapsed_cycles();
            // First receive package size header
            union {
                uint32_t header_word;
                uint8_t header_bytes[4];
            };
            for(size_t i = 0; i < sizeof(header_bytes); ) {
                // cout << "starting receive1...\n";
                __m3_sysc_trace_start(SYSC_RECEIVE);
                ssize_t res = socket->recv(header_bytes + i, sizeof(header_bytes) - i);
                __m3_sysc_trace_stop();
                // cout << "stopped receive1.\n";
                i += static_cast<size_t>(res);
            }

            uint32_t package_size = be32toh(header_word);
            if(package_size > sizeof(package_buffer)) {
                cerr << "Invalid package header length " << package_size << "\n";
                return 1;
            }

            // Receive the next package from the socket
            for(size_t i = 0; i < package_size; ) {
                // cout << "starting receive2...\n";
                __m3_sysc_trace_start(SYSC_RECEIVE);
                ssize_t res = socket->recv(package_buffer + i, package_size - i);
                __m3_sysc_trace_stop();
                // cout << "stopped receive2.\n";
                i += static_cast<size_t>(res);
            }

            recv_timing += m3::CPU::elapsed_cycles() - recv_start;

            // There is an edge case where the package size is 6, If thats the case, check if we got the
            // end flag from the client. In that case its time to stop the benchmark.
            if(package_size == 6 && memcmp(package_buffer, "ENDNOW", 6) == 0) {
                run = false;
                break;
            }
            if(package_size == 6 && memcmp(package_buffer, "ENDRUN", 6) == 0)
                break;

            uint64_t op_start = m3::CPU::elapsed_cycles();
            Package pkg;
            if(from_bytes(package_buffer, package_size, &pkg)) {
                if((opcounter % 100) == 0)
                    cout << "Op=" << pkg.op << " @ " << opcounter << "\n";

                exec->execute(pkg);
                opcounter += 1;

                if((opcounter % 16) == 0) {
                    //cout << "starting send...\n";
                    uint8_t b = 0;
                    __m3_sysc_trace_start(SYSC_SEND);
                    socket->send(&b, 1);
                    __m3_sysc_trace_stop();
                    //cout << "stopped send.\n";
                }

                op_timing += m3::CPU::elapsed_cycles() - op_start;
            }
        }

        cycles_t end = m3::CPU::elapsed_cycles();
        cout << "Execution took " << (end - start) << " cycles\n";
    }

    cout << "Server Side:\n";
    cout << "     avg recv time: " << (recv_timing / opcounter) << " cycles\n";
    cout << "     avg op time  : " << (op_timing / opcounter) << " cycles\n";
    exec->print_stats(opcounter);

#if defined(__kachel__)
    __m3_sysc_trace(false, 0);
#endif

    return 0;
}
