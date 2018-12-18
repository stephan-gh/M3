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

#include <m3/stream/Standard.h>

#include <m3/vfs/FileRef.h>

#include "../cppbench.h"

using namespace m3;

NOINLINE static void creation() {
    Profile pr(4, 2);

    cout << "VPE creation: " << pr.run_with_id([] {
        VPE vpe("hello");
    }, 0x90) << "\n";
}

NOINLINE static void run() {
    const ulong warmup = 2;
    const ulong repeats = 4;

    Results res(warmup + repeats);
    for(ulong i = 0; i < warmup + repeats; ++i) {
        VPE vpe("hello");

        auto start = Time::start(0x91);
        Errors::Code err = vpe.run([start]() {
            cycles_t end = Time::stop(0x91);
            return end - start;
        });
        if(err != Errors::NONE)
            exitmsg("VPE::run failed");

        cycles_t time = static_cast<cycles_t>(vpe.wait());
        if(i >= warmup)
            res.push(time);
    }

    cout << "VPE run: " << res << "\n";
}

NOINLINE static void run_wait() {
    Profile pr(4, 2);

    cout << "VPE run wait: " << pr.run_with_id([] {
        VPE vpe("hello");
        Errors::Code res = vpe.run([]() {
            return 0;
        });
        if(res != Errors::NONE)
            exitmsg("VPE::run failed");
        vpe.wait();
    }, 0x90) << "\n";
}

NOINLINE static void run_multi_wait() {
    Profile pr(4, 2);

    VPE vpe("hello");

    cout << "VPE run multi-wait: " << pr.run_with_id([&vpe] {
        Errors::Code res = vpe.run([]() {
            return 0;
        });
        if(res != Errors::NONE)
            exitmsg("VPE::run failed");
        vpe.wait();
    }, 0x90) << "\n";
}

NOINLINE static void exec() {
    Profile pr(4, 2);

    cout << "VPE exec: " << pr.run_with_id([] {
        VPE vpe("hello");
        const char *args[] = {"/bin/noop"};
        Errors::Code res = vpe.exec(ARRAY_SIZE(args), args);
        if(res != Errors::NONE)
            exitmsg("Unable to load " << args[0]);
        vpe.wait();
    }, 0x90) << "\n";
}

void bvpe() {
    RUN_BENCH(creation);
    RUN_BENCH(run);
    RUN_BENCH(run_wait);
    RUN_BENCH(run_multi_wait);
    RUN_BENCH(exec);
}
