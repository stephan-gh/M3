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

#include <m3/accel/StreamAccel.h>
#include <m3/stream/Standard.h>
#include <m3/pipe/IndirectPipe.h>
#include <m3/vfs/VFS.h>
#include <m3/Syscalls.h>

#include "imgproc.h"

using namespace m3;

static constexpr bool VERBOSE           = 1;
static constexpr size_t PIPE_SHM_SIZE   = 512 * 1024;

static const char *names[] = {
    "FFT",
    "MUL",
    "IFFT",
};

class DirectChain {
public:
    static const size_t ACCEL_COUNT     = 3;

    explicit DirectChain(Pipes &pipesrv, size_t id,
                         FileRef<GenericFile> &in, FileRef<GenericFile> &out, Mode _mode)
        : mode(_mode),
          acts(),
          accels(),
          pipes(),
          mems() {
        // create activities
        for(size_t i = 0; i < ACCEL_COUNT; ++i) {
            OStringStream name;
            name << names[i] << id;

            if(VERBOSE) Serial::get() << "Creating Activity " << name.str() << "\n";

            tiles[i] = Tile::get("copy");
            acts[i] = std::make_unique<ChildActivity>(tiles[i], name.str());

            accels[i] = std::make_unique<StreamAccel>(acts[i], ACCEL_TIMES[i]);

            if(mode == Mode::DIR_SIMPLE && i + 1 < ACCEL_COUNT) {
                mems[i] = std::make_unique<MemGate>(
                    MemGate::create_global(PIPE_SHM_SIZE, MemGate::RW));
                pipes[i] = std::make_unique<IndirectPipe>(
                    pipesrv, *mems[i], PIPE_SHM_SIZE);
            }
        }

        if(VERBOSE) Serial::get() << "Connecting input and output...\n";

        // connect input/output
        accels[0]->connect_input(&*in);
        accels[ACCEL_COUNT - 1]->connect_output(&*out);
        for(size_t i = 0; i < ACCEL_COUNT; ++i) {
            if(i > 0) {
                if(mode == Mode::DIR_SIMPLE) {
                    auto rd = Activity::own().files()->get(pipes[i - 1]->reader_fd());
                    accels[i]->connect_input(static_cast<GenericFile*>(rd));
                }
                else
                    accels[i]->connect_input(accels[i - 1].get());
            }
            if(i + 1 < ACCEL_COUNT) {
                if(mode == Mode::DIR_SIMPLE) {
                    auto wr = Activity::own().files()->get(pipes[i]->writer_fd());
                    accels[i]->connect_output(static_cast<GenericFile*>(wr));
                }
                else
                    accels[i]->connect_output(accels[i + 1].get());
            }
        }
    }

    void start() {
        for(size_t i = 0; i < ACCEL_COUNT; ++i) {
            acts[i]->start();
            running[i] = true;
        }
    }

    void add_running(capsel_t *sels, size_t *count) {
        for(size_t i = 0; i < ACCEL_COUNT; ++i) {
            if(running[i])
                sels[(*count)++] = acts[i]->sel();
        }
    }
    void terminated(capsel_t act, int exitcode) {
        for(size_t i = 0; i < ACCEL_COUNT; ++i) {
            if(running[i] && acts[i]->sel() == act) {
                if(exitcode != 0) {
                    cerr << "chain" << i
                         << " terminated with exit code " << exitcode << "\n";
                }
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
    Mode mode;
    Reference<Tile> tiles[ACCEL_COUNT];
    std::unique_ptr<ChildActivity> acts[ACCEL_COUNT];
    std::unique_ptr<StreamAccel> accels[ACCEL_COUNT];
    std::unique_ptr<IndirectPipe> pipes[ACCEL_COUNT];
    std::unique_ptr<MemGate> mems[ACCEL_COUNT];
    bool running[ACCEL_COUNT];
};

static void wait_for(std::unique_ptr<DirectChain> *chains, size_t num) {
    for(size_t rem = num * DirectChain::ACCEL_COUNT; rem > 0; --rem) {
        size_t count = 0;
        capsel_t sels[num * DirectChain::ACCEL_COUNT];
        for(size_t i = 0; i < num; ++i)
            chains[i]->add_running(sels, &count);

        capsel_t act;
        int exitcode = Syscalls::activity_wait(sels, rem, 0, &act);
        for(size_t i = 0; i < num; ++i)
            chains[i]->terminated(act, exitcode);
    }
}

CycleDuration chain_direct(const char *in, size_t num, Mode mode) {
    Pipes pipes("pipes");
    std::unique_ptr<DirectChain> chains[num];
    FileRef<GenericFile> infds[num];
    FileRef<GenericFile> outfds[num];

    // create <num> chains
    for(size_t i = 0; i < num; ++i) {
        OStringStream outpath;
        outpath << "/tmp/res-" << i;

        infds[i] = VFS::open(in, FILE_R | FILE_NEWSESS);
        outfds[i] = VFS::open(outpath.str(), FILE_W | FILE_TRUNC | FILE_CREATE | FILE_NEWSESS);

        chains[i] = std::make_unique<DirectChain>(pipes,
                                                  i,
                                                  infds[i],
                                                  outfds[i],
                                                  mode);
    }

    if(VERBOSE) Serial::get() << "Starting chain...\n";

    auto start = CycleInstant::now();

    if(mode == Mode::DIR) {
        for(size_t i = 0; i < num; ++i)
            chains[i]->start();
        wait_for(chains, num);
    }
    else {
        for(size_t i = 0; i < num / 2; ++i)
            chains[i]->start();
        wait_for(chains, num / 2);
        for(size_t i = num / 2; i < num; ++i)
            chains[i]->start();
        wait_for(chains + num / 2, num / 2);
    }

    auto end = CycleInstant::now();

    return end.duration_since(start);
}
