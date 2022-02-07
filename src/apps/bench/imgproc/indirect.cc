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
#include <base/time/Instant.h>
#include <base/TileDesc.h>

#include <m3/accel/InDirAccel.h>
#include <m3/stream/Standard.h>
#include <m3/session/Pager.h>
#include <m3/vfs/VFS.h>
#include <m3/Syscalls.h>

using namespace m3;

#include "imgproc.h"

static const bool VERBOSE           = 0;
static const size_t BUF_SIZE        = 2048;
static const size_t REPLY_SIZE      = 64;

static constexpr size_t ACCEL_COUNT = 3;

struct IndirChain {
    explicit IndirChain(size_t _id, RecvGate &_reply_gate, Reference<File> _in, Reference<File> _out)
        : id(_id),
          in(_in),
          out(_out),
          total(),
          seen(),
          reply_gate(_reply_gate),
          sizes(),
          acts(),
          accels(),
          ops() {
        for(size_t i = 0; i < ACCEL_COUNT; ++i) {
            OStringStream name;
            name << "chain" << id << "-" << i;

            tiles[i] = Tile::get("indir");
            acts[i] = std::make_unique<Activity>(tiles[i], name.str());

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
            accels[1]->start(InDirAccel::Operation::COMPUTE, written, ACCEL_TIMES[1], idx_to_label(1));
            ops[1] = InDirAccel::Operation::COMPUTE;

            read_next(buffer);
        }
        else {
            accels[idx + 1]->start(InDirAccel::Operation::COMPUTE, written,
                                   ACCEL_TIMES[idx + 1], idx_to_label(idx + 1));
            ops[idx + 1] = InDirAccel::Operation::COMPUTE;
        }

        if(sizes[idx] > 0) {
            accels[idx - 1]->start(InDirAccel::Operation::FORWARD, sizes[idx],
                                   ACCEL_TIMES[idx - 1], idx_to_label(idx - 1));
            ops[idx - 1] = InDirAccel::Operation::FORWARD;
            sizes[idx] = 0;
        }

        if(VERBOSE) cout << "chain" << id << ": " << seen << " / " << total << "\n";
        return seen < total;
    }

    bool read_next(void *buffer) {
        size_t count = in->read(buffer, BUF_SIZE);
        if(count == 0)
            return false;

        accels[0]->write(buffer, static_cast<size_t>(count));
        accels[0]->start(InDirAccel::Operation::COMPUTE,
                         static_cast<size_t>(count),
                         ACCEL_TIMES[0],
                         idx_to_label(0));
        ops[0] = InDirAccel::Operation::COMPUTE;
        total += static_cast<size_t>(count);
        return true;
    }

    size_t id;
    Reference<File> in;
    Reference<File> out;
    size_t total;
    size_t seen;
    RecvGate &reply_gate;
    size_t sizes[ACCEL_COUNT];
    Reference<Tile> tiles[ACCEL_COUNT];
    std::unique_ptr<Activity> acts[ACCEL_COUNT];
    std::unique_ptr<InDirAccel> accels[ACCEL_COUNT];
    InDirAccel::Operation ops[ACCEL_COUNT];
};

CycleDuration chain_indirect(const char *in, size_t num) {
    std::unique_ptr<uint8_t[]> buffer(new uint8_t[BUF_SIZE]);

    RecvGate reply_gate = RecvGate::create(getnextlog2(REPLY_SIZE * num * ACCEL_COUNT),
                                           nextlog2<REPLY_SIZE>::val);
    reply_gate.activate();

    fd_t infds[num];
    fd_t outfds[num];
    std::unique_ptr<IndirChain> chains[num];

    // create chains
    for(size_t i = 0; i < num; ++i) {
        OStringStream outpath;
        outpath << "/tmp/res-" << i;

        infds[i] = VFS::open(in, FILE_R);
        outfds[i] = VFS::open(outpath.str(), FILE_W | FILE_TRUNC | FILE_CREATE);

        chains[i] = std::unique_ptr<IndirChain>(
            new IndirChain(i, reply_gate, Activity::self().files()->get(infds[i]),
                                          Activity::self().files()->get(outfds[i]))
        );
    }

    auto start = CycleInstant::now();
    auto end = start;

    // start chains
    for(size_t i = 0; i < num; ++i)
        chains[i]->start();

    size_t active_chains = 0;
    for(size_t i = 0; i < num; ++i) {
        if(!chains[i]->read_next(buffer.get()))
            goto error;
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

        if(VERBOSE) cout << "message for chain" << chain << ", accel" << accel << "\n";

        if(!chains[chain]->handle_msg(buffer.get(), accel, written))
            active_chains &= ~(static_cast<size_t>(1) << chain);
    }

    end = CycleInstant::now();

error:
    for(size_t i = 0; i < num; ++i) {
        VFS::close(infds[i]);
        VFS::close(outfds[i]);
    }

    return end.duration_since(start);
}
