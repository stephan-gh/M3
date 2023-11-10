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

NOINLINE static void creation() {
    Profile pr(4, 2);

    auto tile = Tile::get("compat|own");
    WVPERF("Activity creation", pr.run<CycleInstant>([&tile] {
        ChildActivity act(tile, "hello");
    }));
}

NOINLINE static void run() {
    const ulong warmup = 2;
    const ulong repeats = 4;

    auto rgate = RecvGate::create(nextlog2<256>::val, nextlog2<256>::val);
    rgate.activate();
    auto scap = SendCap::create(&rgate, SendGateArgs().credits(SendGate::UNLIMITED));

    auto tile = Tile::get("compat|own");
    Results<CycleDuration> res(warmup + repeats);
    for(ulong i = 0; i < warmup + repeats; ++i) {
        ChildActivity act(tile, "hello");

        capsel_t scap_sel = scap.sel();
        act.delegate_obj(scap_sel);

        auto start = CycleInstant::now();
        act.data_sink() << start.as_cycles() << scap_sel;

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

    auto tile = Tile::get("compat|own");
    WVPERF("Activity run wait", pr.run<CycleInstant>([&tile] {
        ChildActivity act(tile, "hello");
        act.run([]() {
            return 0;
        });
        act.wait();
    }));
}

NOINLINE static void exec() {
    Profile pr(4, 2);

    auto tile = Tile::get("compat|own");
    WVPERF("Activity exec", pr.run<CycleInstant>([&tile] {
        ChildActivity act(tile, "hello");
#if defined(__m3lx__)
        const char *args[] = {"/bin/true"};
#else
        const char *args[] = {"/bin/noop"};
#endif
        act.exec(ARRAY_SIZE(args), args);
        act.wait();
    }));
}

void bactivity() {
    RUN_BENCH(creation);
    RUN_BENCH(run);
    RUN_BENCH(run_wait);
    RUN_BENCH(exec);
}
