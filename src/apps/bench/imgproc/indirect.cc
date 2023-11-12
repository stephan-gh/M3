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

#include <base/Common.h>
#include <base/TileDesc.h>
#include <base/time/Instant.h>

#include <m3/Syscalls.h>
#include <m3/accel/InDirAccel.h>
#include <m3/session/Pager.h>
#include <m3/stream/Standard.h>
#include <m3/vfs/VFS.h>

using namespace m3;

#include "imgproc.h"

static const int VERBOSE = 1;
static const size_t BUF_SIZE = 2048;
static const size_t REPLY_SIZE = 64;

static constexpr size_t ACCEL_COUNT = 3;

struct IndirChain {
    explicit IndirChain(size_t _id, RecvGate &_reply_gate, FileRef<GenericFile> _in,
                        FileRef<GenericFile> _out)
        : id(_id),
          in(std::move(_in)),
          out(std::move(_out)),
          total(),
          seen(),
          reply_gate(_reply_gate),
          sizes(),
          acts(),
          accels(),
          ops() {
        for(size_t i = 0; i < ACCEL_COUNT; ++i) {
            OStringStream name;
            format_to(name, "chain{}-{}"_cf, id, i);

            if(VERBOSE)
                println("Creating Activity {}"_cf, name.str());

            tiles[i] = Tile::get("indir");
            acts[i] = std::make_unique<ChildActivity>(tiles[i], name.str());

            accels[i] = std::make_unique<InDirAccel>(acts[i], reply_gate);
            ops[i] = InDirAccel::Operation::IDLE;
        }

        for(size_t i = 0; i < ACCEL_COUNT - 1; ++i)
            accels[i]->connect_output(accels[i + 1].get());
    }

    label_t idx_to_label(size_t i) const {
        // label 0 is special; use 1..n
        return 1 + (id * ACCEL_COUNT) + i;
    }

    void start() {
        for(size_t i = 0; i < ACCEL_COUNT; ++i)
            acts[i]->start();
    }

    bool handle_msg(void *buffer, size_t idx, size_t written) {
        if(idx < ACCEL_COUNT - 1 && ops[idx] == InDirAccel::Operation::COMPUTE) {
            if(ops[idx + 1] == InDirAccel::Operation::IDLE) {
                ops[idx] = InDirAccel::Operation::FORWARD;
                accels[idx]->start(InDirAccel::Operation::FORWARD, written,
                                   CycleDuration::from_raw(0), idx_to_label(idx));
            }
            else
                sizes[idx + 1] = written;
            return true;
        }

        ops[idx] = InDirAccel::Operation::IDLE;

        if(idx == ACCEL_COUNT - 1) {
            accels[idx]->read(buffer, written);
            out->write(buffer, written);
            seen += written;
        }
        else if(idx == 0) {
            accels[1]->start(InDirAccel::Operation::COMPUTE, written, ACCEL_TIMES[1],
                             idx_to_label(1));
            ops[1] = InDirAccel::Operation::COMPUTE;

            read_next(buffer);
        }
        else {
            accels[idx + 1]->start(InDirAccel::Operation::COMPUTE, written, ACCEL_TIMES[idx + 1],
                                   idx_to_label(idx + 1));
            ops[idx + 1] = InDirAccel::Operation::COMPUTE;
        }

        if(sizes[idx] > 0) {
            accels[idx - 1]->start(InDirAccel::Operation::FORWARD, sizes[idx], ACCEL_TIMES[idx - 1],
                                   idx_to_label(idx - 1));
            ops[idx - 1] = InDirAccel::Operation::FORWARD;
            sizes[idx] = 0;
        }

        if(VERBOSE > 1)
            println("chain{}: seen {} / {}"_cf, id, seen, total);
        return seen < total;
    }

    bool read_next(void *buffer) {
        size_t count = in->read(buffer, BUF_SIZE).unwrap();
        if(count == 0)
            return false;

        accels[0]->write(buffer, count);
        accels[0]->start(InDirAccel::Operation::COMPUTE, count, ACCEL_TIMES[0], idx_to_label(0));
        ops[0] = InDirAccel::Operation::COMPUTE;
        total += count;
        return true;
    }

    size_t id;
    FileRef<GenericFile> in;
    FileRef<GenericFile> out;
    size_t total;
    size_t seen;
    RecvGate &reply_gate;
    size_t sizes[ACCEL_COUNT];
    Reference<Tile> tiles[ACCEL_COUNT];
    std::unique_ptr<ChildActivity> acts[ACCEL_COUNT];
    std::unique_ptr<InDirAccel> accels[ACCEL_COUNT];
    InDirAccel::Operation ops[ACCEL_COUNT];
};

CycleDuration chain_indirect(const char *in, size_t num) {
    std::unique_ptr<uint8_t[]> buffer(new uint8_t[BUF_SIZE]);

    RecvGate reply_gate =
        RecvGate::create(getnextlog2(REPLY_SIZE * num * ACCEL_COUNT), nextlog2<REPLY_SIZE>::val);

    FileRef<GenericFile> infds[num];
    FileRef<GenericFile> outfds[num];
    std::unique_ptr<IndirChain> chains[num];

    // create chains
    for(size_t i = 0; i < num; ++i) {
        OStringStream outpath;
        format_to(outpath, "/tmp/res-{}"_cf, i);

        infds[i] = VFS::open(in, FILE_R);
        outfds[i] = VFS::open(outpath.str(), FILE_W | FILE_TRUNC | FILE_CREATE);

        chains[i] = std::unique_ptr<IndirChain>(
            new IndirChain(i, reply_gate, std::move(infds[i]), std::move(outfds[i])));
    }

    if(VERBOSE)
        println("Starting chain..."_cf);

    auto start = CycleInstant::now();

    // start chains
    for(size_t i = 0; i < num; ++i)
        chains[i]->start();

    size_t active_chains = 0;
    for(size_t i = 0; i < num; ++i) {
        if(!chains[i]->read_next(buffer.get()))
            vthrow(Errors::END_OF_FILE, "Unexpected end of file"_cf);
        active_chains |= static_cast<size_t>(1) << i;
    }

    while(active_chains != 0) {
        label_t label;
        size_t written;

        // ack the message immediately
        {
            GateIStream is = receive_msg(reply_gate);
            label = is.label<label_t>();
            is >> written;
        }

        size_t chain = (label - 1) / ACCEL_COUNT;
        size_t accel = (label - 1) % ACCEL_COUNT;

        if(VERBOSE > 1)
            println("message for chain{}, accel{}"_cf, chain, accel);

        if(!chains[chain]->handle_msg(buffer.get(), accel, written))
            active_chains &= ~(static_cast<size_t>(1) << chain);
    }

    return CycleInstant::now().duration_since(start);
}
