/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

#include <base/stream/Serial.h>
#include <base/time/Instant.h>

#include <m3/Syscalls.h>
#include <m3/accel/StreamAccel.h>
#include <m3/pipe/IndirectPipe.h>
#include <m3/stream/Standard.h>
#include <m3/vfs/VFS.h>

#include "accelchain.h"

using namespace m3;

static constexpr bool VERBOSE = 1;
static constexpr size_t PIPE_SHM_SIZE = 512 * 1024;

class Chain {
    static const size_t MAX_NUM = 8;

public:
    explicit Chain(Pipes &pipesrv, FileRef<GenericFile> &in, FileRef<GenericFile> &out, size_t _num,
                   CycleDuration comptime, Mode _mode)
        : num(_num),
          mode(_mode),
          acts(),
          accels(),
          pipes(),
          mems() {
        // create activities
        for(size_t i = 0; i < num; ++i) {
            OStringStream name;
            format_to(name, "chain{}"_cf, i);

            if(VERBOSE)
                println("Creating Activity {}"_cf, name.str());

            tiles[i] = Tile::get("copy");
            acts[i] = std::make_unique<ChildActivity>(tiles[i], name.str());

            accels[i] = std::make_unique<StreamAccel>(acts[i], comptime);

            if(mode == Mode::DIR_SIMPLE && i + 1 < num) {
                mems[i] =
                    std::make_unique<MemCap>(MemCap::create_global(PIPE_SHM_SIZE, MemCap::RW));
                pipes[i] = std::make_unique<IndirectPipe>(pipesrv, *mems[i], PIPE_SHM_SIZE);
            }
        }

        if(VERBOSE)
            println("Connecting input and output..."_cf);

        // connect input/output
        accels[0]->connect_input(&*in);
        accels[num - 1]->connect_output(&*out);
        for(size_t i = 0; i < num; ++i) {
            if(i > 0) {
                if(mode == Mode::DIR_SIMPLE) {
                    auto &rd = pipes[i - 1]->reader();
                    accels[i]->connect_input(&rd);
                }
                else
                    accels[i]->connect_input(accels[i - 1].get());
            }
            if(i + 1 < num) {
                if(mode == Mode::DIR_SIMPLE) {
                    auto &wr = pipes[i]->writer();
                    accels[i]->connect_output(&wr);
                }
                else
                    accels[i]->connect_output(accels[i + 1].get());
            }
        }
    }

    void start() {
        for(size_t i = 0; i < num; ++i) {
            acts[i]->start();
            running[i] = true;
        }
    }

    void add_running(capsel_t *sels, size_t *count) {
        for(size_t i = 0; i < num; ++i) {
            if(running[i])
                sels[(*count)++] = acts[i]->sel();
        }
    }
    void terminated(capsel_t act, int exitcode) {
        for(size_t i = 0; i < num; ++i) {
            if(running[i] && acts[i]->sel() == act) {
                if(exitcode != 0)
                    eprintln("chain{} terminated with exit code {}"_cf, i, exitcode);
                if(mode == Mode::DIR_SIMPLE) {
                    if(pipes[i])
                        pipes[i]->close_writer();
                    if(i > 0 && pipes[i - 1])
                        pipes[i - 1]->close_reader();
                }
                running[i] = false;
                break;
            }
        }
    }

private:
    size_t num;
    Mode mode;
    Reference<Tile> tiles[MAX_NUM];
    std::unique_ptr<ChildActivity> acts[MAX_NUM];
    std::unique_ptr<StreamAccel> accels[MAX_NUM];
    std::unique_ptr<IndirectPipe> pipes[MAX_NUM];
    std::unique_ptr<MemCap> mems[MAX_NUM];
    bool running[MAX_NUM];
};

void chain_direct(FileRef<GenericFile> &in, FileRef<GenericFile> &out, size_t num,
                  CycleDuration comptime, Mode mode) {
    Pipes pipes("pipes");
    Chain ch(pipes, in, out, num, comptime, mode);

    if(VERBOSE)
        println("Starting chain..."_cf);

    auto start = CycleInstant::now();

    ch.start();

    // wait for their completion
    for(size_t rem = num; rem > 0; --rem) {
        size_t count = 0;
        capsel_t sels[num];
        ch.add_running(sels, &count);

        const auto [exitcode, act] = Syscalls::activity_wait(sels, rem, 0);
        ch.terminated(act, exitcode);
    }

    auto end = CycleInstant::now();
    println("Total time: {}"_cf, end.duration_since(start));
}

void chain_direct_multi(FileRef<GenericFile> &in, FileRef<GenericFile> &out, size_t num,
                        CycleDuration comptime, Mode mode) {
    Pipes pipes("pipes");
    Chain ch1(pipes, in, out, num, comptime, mode);

    auto out2 = VFS::open("/tmp/out2.txt", FILE_W | FILE_TRUNC | FILE_CREATE | FILE_NEWSESS);
    auto in2 = FileRef<GenericFile>(in->clone());
    Chain ch2(pipes, in2, out2, num, comptime, mode);

    if(VERBOSE)
        println("Starting chains..."_cf);

    auto start = CycleInstant::now();

    ch1.start();
    ch2.start();

    // wait for their completion
    for(size_t rem = num * 2; rem > 0; --rem) {
        size_t count = 0;
        capsel_t sels[num * 2];
        ch1.add_running(sels, &count);
        ch2.add_running(sels, &count);

        const auto [exitcode, act] = Syscalls::activity_wait(sels, rem, 0);
        ch1.terminated(act, exitcode);
        ch2.terminated(act, exitcode);
    }

    auto end = CycleInstant::now();
    println("Total time: {}"_cf, end.duration_since(start));
}
