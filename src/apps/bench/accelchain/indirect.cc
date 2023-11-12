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
#include <base/stream/IStringStream.h>
#include <base/time/Instant.h>

#include <m3/Syscalls.h>
#include <m3/accel/InDirAccel.h>
#include <m3/session/Pager.h>
#include <m3/stream/Standard.h>
#include <m3/vfs/VFS.h>

#include <memory>

#include "accelchain.h"

using namespace m3;

static const size_t BUF_SIZE = 4096;
static const size_t REPLY_SIZE = 64;

void chain_indirect(FileRef<GenericFile> &in, FileRef<GenericFile> &out, size_t num,
                    CycleDuration comptime) {
    std::unique_ptr<uint8_t> buffer(new uint8_t[BUF_SIZE]);

    Reference<Tile> tiles[num];
    std::unique_ptr<ChildActivity> acts[num];
    std::unique_ptr<InDirAccel> accels[num];
    InDirAccel::Operation ops[num];

    RecvGate reply_gate =
        RecvGate::create(getnextlog2(REPLY_SIZE * num), nextlog2<REPLY_SIZE>::val);

    // create activities
    for(size_t i = 0; i < num; ++i) {
        OStringStream name;
        format_to(name, "chain{}"_cf, i);

        tiles[i] = Tile::get("indir");
        acts[i] = std::make_unique<ChildActivity>(tiles[i], name.str());

        accels[i] = std::make_unique<InDirAccel>(acts[i], reply_gate);
    }

    // connect outputs
    for(size_t i = 0; i < num - 1; ++i)
        accels[i]->connect_output(accels[i + 1].get());

    auto start = CycleInstant::now();

    // start activities
    for(size_t i = 0; i < num; ++i)
        acts[i]->start();

    size_t total = 0, seen = 0;
    size_t count = in->read(buffer.get(), BUF_SIZE).unwrap();

    // label 0 is special; use 1..n
    accels[0]->write(buffer.get(), count);
    accels[0]->start(InDirAccel::Operation::COMPUTE, count, comptime, 1);
    ops[0] = InDirAccel::Operation::COMPUTE;
    total += count;

    count = in->read(buffer.get(), BUF_SIZE).unwrap();

    while(seen < total) {
        label_t label;
        size_t written;

        // ack the message immediately
        {
            GateIStream is = receive_msg(reply_gate);
            label = is.label<label_t>() - 1;
            is >> written;
        }

        // cout << "got msg from " << label << "\n";

        if(ops[label] == InDirAccel::Operation::COMPUTE) {
            ops[label] = InDirAccel::Operation::FORWARD;
            accels[label]->start(InDirAccel::Operation::FORWARD, written,
                                 CycleDuration::from_raw(0), label + 1);
            continue;
        }

        if(label == num - 1) {
            accels[num - 1]->read(buffer.get(), written);
            // cout << "write " << written << " bytes\n";
            out->write(buffer.get(), written);
            seen += written;
        }

        if(label == 0) {
            if(num > 1) {
                accels[1]->start(InDirAccel::Operation::COMPUTE, written, comptime, 2);
                ops[1] = InDirAccel::Operation::COMPUTE;
            }

            total += static_cast<size_t>(count);
            if(count > 0) {
                accels[0]->write(buffer.get(), static_cast<size_t>(count));
                accels[0]->start(InDirAccel::Operation::COMPUTE, static_cast<size_t>(count),
                                 comptime, 1);
                ops[0] = InDirAccel::Operation::COMPUTE;

                count = in->read(buffer.get(), BUF_SIZE).unwrap();
                // cout << "read " << count << " bytes\n";
            }
        }
        else if(label != num - 1) {
            accels[label + 1]->start(InDirAccel::Operation::COMPUTE, written, comptime,
                                     label + 1 + 1);
            ops[label + 1] = InDirAccel::Operation::COMPUTE;
        }

        // cout << seen << " / " << total << "\n";
    }

    auto end = CycleInstant::now();
    println("Total time: {}"_cf, end.duration_since(start));
}
