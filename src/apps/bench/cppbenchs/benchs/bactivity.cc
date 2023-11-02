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
#include <base/Panic.h>
#include <base/time/Profile.h>

#include <m3/Test.h>
#include <m3/com/GateStream.h>
#include <m3/tiles/ChildActivity.h>
#include <m3/vfs/FileRef.h>

#include "../cppbenchs.h"

using namespace m3;

// TODO workaround until "compat" respects the multiplexer
#if defined(__m3lx__)
static const char *CHILD_TILE = "own";
#else
static const char *CHILD_TILE = "compat|own";
#endif

NOINLINE static void creation() {
    Profile pr(4, 2);

    auto tile = Tile::get(CHILD_TILE);
    WVPERF("Activity creation", pr.run<CycleInstant>([&tile] {
        ChildActivity act(tile, "hello");
    }));
}

NOINLINE static void run() {
    const ulong warmup = 2;
    const ulong repeats = 4;

    auto rgate = RecvGate::create(nextlog2<256>::val, nextlog2<256>::val);
    rgate.activate();
    auto sgate = SendGate::create(&rgate, SendGateArgs().credits(SendGate::UNLIMITED));

    auto tile = Tile::get(CHILD_TILE);
    Results<CycleDuration> res(warmup + repeats);
    for(ulong i = 0; i < warmup + repeats; ++i) {
        ChildActivity act(tile, "hello");

        capsel_t sgate_sel = sgate.sel();
        act.delegate_obj(sgate_sel);

        auto start = CycleInstant::now();
        act.data_sink() << start.as_cycles() << sgate_sel;

        act.run([]() {
            capsel_t sgate_sel;
            uint64_t start;
            Activity::own().data_source() >> start >> sgate_sel;

            auto sgate = SendGate::bind(sgate_sel);
            auto end = CycleInstant::now();
            send_vmsg(sgate, end.duration_since(CycleInstant::from_cycles(start)).as_raw());
            return 0;
        });

        auto reply = receive_msg(rgate);
        cycles_t time;
        reply >> time;
        if(i >= warmup)
            res.push(CycleDuration::from_raw(time));
        WVASSERTEQ(act.wait(), 0);
    }

    WVPERF("Activity run", res);
}

NOINLINE static void run_wait() {
    Profile pr(4, 2);

    auto tile = Tile::get(CHILD_TILE);
    WVPERF("Activity run wait", pr.run<CycleInstant>([&tile] {
        ChildActivity act(tile, "hello");
        act.run([]() {
            return 0;
        });
        act.wait();
    }));
}

#if !defined(__m3lx__)
NOINLINE static void exec() {
    Profile pr(4, 2);

    auto tile = Tile::get("core|own");
    WVPERF("Activity exec", pr.run<CycleInstant>([&tile] {
        ChildActivity act(tile, "hello");
        const char *args[] = {"/bin/noop"};
        act.exec(ARRAY_SIZE(args), args);
        act.wait();
    }));
}
#endif

void bactivity() {
    RUN_BENCH(creation);
    RUN_BENCH(run);
    RUN_BENCH(run_wait);
#if !defined(__m3lx__)
    RUN_BENCH(exec);
#endif
}
