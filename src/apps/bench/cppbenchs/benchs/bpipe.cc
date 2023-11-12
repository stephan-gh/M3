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
#include <base/KIF.h>
#include <base/Panic.h>
#include <base/col/SList.h>
#include <base/time/Profile.h>

#include <m3/Syscalls.h>
#include <m3/Test.h>
#include <m3/pipe/IndirectPipe.h>
#include <m3/tiles/ChildActivity.h>

#include "../cppbenchs.h"

using namespace m3;

const size_t DATA_SIZE = 2 * 1024 * 1024;
const size_t BUF_SIZE = 8 * 1024;

alignas(PAGE_SIZE) static char buf[BUF_SIZE];

NOINLINE void child_to_parent() {
    Profile pr(2, 1);

    auto res = pr.run<CycleInstant>([] {
        Pipes pipes("pipes");
        MemCap mgate = MemCap::create_global(0x1'0000, MemCap::RW);
        IndirectPipe pipe(pipes, mgate, 0x1'0000);

        Reference<Tile> tile = Tile::get("compat|own");
        ChildActivity act(tile, "writer");
        act.add_file(STDOUT_FD, pipe.writer().fd());

        act.run([] {
            auto output = Activity::own().files()->get(STDOUT_FD);
            auto rem = DATA_SIZE;
            while(rem > 0) {
                output->write(buf, sizeof(buf));
                rem -= sizeof(buf);
            }
            return 0;
        });

        pipe.close_writer();

        auto input = Activity::own().files()->get(pipe.reader().fd());
        while(input->read(buf, sizeof(buf)).unwrap() > 0)
            ;

        pipe.close_reader();

        act.wait();
    });

    auto name = OStringStream();
    format_to(name, "c->p: {} KiB transfer with {} KiB buf"_cf, DATA_SIZE / 1024, BUF_SIZE / 1024);
    WVPERF(name.str(), res);
}

NOINLINE void parent_to_child() {
    Profile pr(2, 1);

    auto res = pr.run<CycleInstant>([] {
        Pipes pipes("pipes");
        MemCap mgate = MemCap::create_global(0x1'0000, MemCap::RW);
        IndirectPipe pipe(pipes, mgate, 0x1'0000);

        Reference<Tile> tile(Tile::get("compat|own"));
        ChildActivity act(tile, "writer");
        act.add_file(STDIN_FD, pipe.reader().fd());

        act.run([] {
            auto input = Activity::own().files()->get(STDIN_FD);
            while(input->read(buf, sizeof(buf)).unwrap() > 0)
                ;
            return 0;
        });

        pipe.close_reader();

        auto output = Activity::own().files()->get(pipe.writer().fd());
        auto rem = DATA_SIZE;
        while(rem > 0) {
            output->write(buf, sizeof(buf));
            rem -= sizeof(buf);
        }

        pipe.close_writer();

        act.wait();
    });

    auto name = OStringStream();
    format_to(name, "p->c: {} KiB transfer with {} KiB buf"_cf, DATA_SIZE / 1024, BUF_SIZE / 1024);
    WVPERF(name.str(), res);
}

void bpipe() {
    RUN_BENCH(child_to_parent);
    RUN_BENCH(parent_to_child);
}
