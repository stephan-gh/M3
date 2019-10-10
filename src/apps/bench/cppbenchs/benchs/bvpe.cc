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
#include <base/util/Profile.h>
#include <base/Panic.h>

#include <m3/vfs/FileRef.h>
#include <m3/Test.h>

#include "../cppbenchs.h"

using namespace m3;

NOINLINE static void creation() {
    Profile pr(4, 2);

    PE pe = PE::alloc(VPE::self().pe_desc());
    WVPERF("VPE creation", pr.run_with_id([&pe] {
        VPE vpe(pe, "hello");
    }, 0x90));
}

NOINLINE static void run() {
    const ulong warmup = 2;
    const ulong repeats = 4;

    PE pe = PE::alloc(VPE::self().pe_desc());
    Results res(warmup + repeats);
    for(ulong i = 0; i < warmup + repeats; ++i) {
        VPE vpe(pe, "hello");

        auto start = Time::start(0x91);
        vpe.run([start]() {
            cycles_t end = Time::stop(0x91);
            return end - start;
        });

        cycles_t time = static_cast<cycles_t>(vpe.wait());
        if(i >= warmup)
            res.push(time);
    }

    WVPERF("VPE run", res);
}

NOINLINE static void run_wait() {
    Profile pr(4, 2);

    PE pe = PE::alloc(VPE::self().pe_desc());
    WVPERF("VPE run wait", pr.run_with_id([&pe] {
        VPE vpe(pe, "hello");
        vpe.run([]() {
            return 0;
        });
        vpe.wait();
    }, 0x90));
}

NOINLINE static void exec() {
    Profile pr(4, 2);

    PE pe = PE::alloc(VPE::self().pe_desc());
    WVPERF("VPE exec", pr.run_with_id([&pe] {
        VPE vpe(pe, "hello");
        const char *args[] = {"/bin/noop"};
        vpe.exec(ARRAY_SIZE(args), args);
        vpe.wait();
    }, 0x90));
}

void bvpe() {
    RUN_BENCH(creation);
    RUN_BENCH(run);
    RUN_BENCH(run_wait);
    RUN_BENCH(exec);
}
