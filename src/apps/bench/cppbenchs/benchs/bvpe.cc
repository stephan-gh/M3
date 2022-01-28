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

#include <base/Common.h>
#include <base/time/Profile.h>
#include <base/Panic.h>

#include <m3/vfs/FileRef.h>
#include <m3/Test.h>

#include "../cppbenchs.h"

using namespace m3;

NOINLINE static void creation() {
    Profile pr(4, 2);

    auto pe = PE::get("core|own");
    WVPERF("VPE creation", pr.run<CycleInstant>([&pe] {
        VPE vpe(pe, "hello");
    }));
}

NOINLINE static void run() {
    const ulong warmup = 2;
    const ulong repeats = 4;

    auto rgate = RecvGate::create(nextlog2<256>::val, nextlog2<256>::val);
    rgate.activate();
    auto sgate = SendGate::create(&rgate, SendGateArgs().credits(SendGate::UNLIMITED));

    auto pe = PE::get("clone|own");
    Results<CycleDuration> res(warmup + repeats);
    for(ulong i = 0; i < warmup + repeats; ++i) {
        VPE vpe(pe, "hello");

        capsel_t sgate_sel = sgate.sel();
        vpe.delegate_obj(sgate_sel);

        auto start = CycleInstant::now();
        vpe.data_sink() << start.as_cycles() << sgate_sel;

        vpe.run([]() {
            capsel_t sgate_sel;
            uint64_t start;
            VPE::self().data_source() >> start >> sgate_sel;

            auto sgate = SendGate::bind(sgate_sel);
            auto end = CycleInstant::now();
            send_vmsg(sgate, end.duration_since(CycleInstant::from_cycles(start)).as_raw());
            return 0;
        });

        if(vpe.wait() == 0) {
            auto reply = receive_msg(rgate);
            cycles_t time;
            reply >> time;
            if(i >= warmup)
                res.push(CycleDuration::from_raw(time));
        }
    }

    WVPERF("VPE run", res);
}

NOINLINE static void run_wait() {
    Profile pr(4, 2);

    auto pe = PE::get("clone|own");
    WVPERF("VPE run wait", pr.run<CycleInstant>([&pe] {
        VPE vpe(pe, "hello");
        vpe.run([]() {
            return 0;
        });
        vpe.wait();
    }));
}

NOINLINE static void exec() {
    Profile pr(4, 2);

    auto pe = PE::get("core|own");
    WVPERF("VPE exec", pr.run<CycleInstant>([&pe] {
        VPE vpe(pe, "hello");
        const char *args[] = {"/bin/noop"};
        vpe.exec(ARRAY_SIZE(args), args);
        vpe.wait();
    }));
}

void bvpe() {
    RUN_BENCH(creation);
    RUN_BENCH(run);
    RUN_BENCH(run_wait);
    RUN_BENCH(exec);
}
