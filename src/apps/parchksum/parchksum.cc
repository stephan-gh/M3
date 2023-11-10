/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/stream/IStringStream.h>

#include <m3/com/GateStream.h>
#include <m3/com/MemGate.h>
#include <m3/com/RecvGate.h>
#include <m3/com/SendGate.h>
#include <m3/stream/Standard.h>
#include <m3/tiles/ChildActivity.h>

using namespace m3;

struct Worker {
    MemGate submem;
    SendCap scap;
    Reference<Tile> tile;
    ChildActivity act;

    Worker(RecvGate &rgate, MemGate &mem, size_t offset, size_t size)
        : submem(mem.derive(offset, size)),
          scap(SendCap::create(&rgate, SendGateArgs().credits(1))),
          tile(Tile::get("compat|own")),
          act(tile, "worker") {
        act.delegate_obj(submem.sel());
    }
};

static const size_t BUF_SIZE = 4096;

int main(int argc, char **argv) {
    size_t memPerAct = 1024 * 1024;
    size_t acts = 2;
    if(argc > 1)
        acts = IStringStream::read_from<size_t>(argv[1]);
    if(argc > 2)
        memPerAct = IStringStream::read_from<size_t>(argv[2]);

    const size_t AREA_SIZE = acts * memPerAct;
    const size_t SUBAREA_SIZE = AREA_SIZE / acts;

    RecvGate rgate = RecvGate::create(getnextlog2(acts * 64), nextlog2<64>::val);
    MemGate mem = MemGate::create_global(AREA_SIZE, MemGate::RW);

    // create worker
    Worker **worker = new Worker *[acts];
    for(size_t i = 0; i < acts; ++i)
        worker[i] = new Worker(rgate, mem, i * SUBAREA_SIZE, SUBAREA_SIZE);

    // write data into memory
    for(size_t i = 0; i < acts; ++i) {
        worker[i]->act.data_sink() << worker[i]->submem.sel() << SUBAREA_SIZE;

        worker[i]->act.run([] {
            capsel_t mem_sel;
            size_t mem_size;
            Activity::own().data_source() >> mem_sel >> mem_size;
            MemGate mem = MemGate::bind(mem_sel);

            uint *buffer = new uint[BUF_SIZE / sizeof(uint)];
            size_t rem = mem_size;
            size_t offset = 0;
            while(rem > 0) {
                for(size_t i = 0; i < BUF_SIZE / sizeof(uint); ++i)
                    buffer[i] = i;
                mem.write(buffer, BUF_SIZE, offset);
                offset += BUF_SIZE;
                rem -= BUF_SIZE;
            }
            println("Memory initialization of {} bytes finished"_cf, mem_size);
            return 0;
        });
    }

    // wait for all workers
    for(size_t i = 0; i < acts; ++i)
        worker[i]->act.wait();

    // now build the checksum
    for(size_t i = 0; i < acts; ++i) {
        worker[i]->act.delegate_obj(worker[i]->scap.sel());

        worker[i]->act.data_sink()
            << worker[i]->submem.sel() << worker[i]->scap.sel() << SUBAREA_SIZE;

        worker[i]->act.run([] {
            capsel_t mem_sel, sgate_sel;
            size_t mem_size;
            Activity::own().data_source() >> mem_sel >> sgate_sel >> mem_size;
            MemGate mem = MemGate::bind(mem_sel);
            SendGate sgate = SendGate::bind(sgate_sel);

            uint *buffer = new uint[BUF_SIZE / sizeof(uint)];

            uint checksum = 0;
            size_t rem = mem_size;
            size_t offset = 0;
            while(rem > 0) {
                mem.read(buffer, BUF_SIZE, offset);
                for(size_t i = 0; i < BUF_SIZE / sizeof(uint); ++i)
                    checksum += buffer[i];
                offset += BUF_SIZE;
                rem -= BUF_SIZE;
            }

            println("Checksum for sub area finished"_cf);
            send_vmsg(sgate, checksum);
            return 0;
        });
    }

    // reduce
    uint checksum = 0;
    for(size_t i = 0; i < acts; ++i) {
        uint actchksum;
        receive_vmsg(rgate, actchksum);
        checksum += actchksum;
    }

    println("Checksum: {}"_cf, checksum);

    for(size_t i = 0; i < acts; ++i) {
        worker[i]->act.wait();
        delete worker[i];
    }
    return 0;
}
