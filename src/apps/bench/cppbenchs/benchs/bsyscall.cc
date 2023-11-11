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
#include <m3/tiles/ChildActivity.h>

#include "../cppbenchs.h"

using namespace m3;

static capsel_t selector = ObjCap::INVALID;

NOINLINE static void noop() {
    Profile pr;
    WVPERF(__func__, pr.run<CycleInstant>([] {
        Syscalls::noop();
    }));
}

NOINLINE static void activate() {
    MemGate mgate = MemGate::create_global(0x1000, MemGate::RW);
    const EP &ep = mgate.activate();

    Profile pr;
    WVPERF(__func__, pr.run<CycleInstant>([&ep, &mgate] {
        Syscalls::activate(ep.sel(), mgate.sel(), KIF::INV_SEL, 0);
    }));
}

NOINLINE static void create_mgate() {
    static uintptr_t addr = Math::round_dn(reinterpret_cast<uintptr_t>(&create_mgate),
                                           static_cast<uintptr_t>(PAGE_SIZE));

    struct SyscallMGateRunner : public Runner {
        void run() override {
            Syscalls::create_mgate(selector, Activity::own().sel(), addr, PAGE_SIZE, KIF::Perm::R);
        }
        void post() override {
            Syscalls::revoke(Activity::own().sel(),
                             KIF::CapRngDesc(KIF::CapRngDesc::OBJ, selector, 1), true);
        }
    };

    Profile pr;
    SyscallMGateRunner runner;
    WVPERF(__func__, pr.runner<CycleInstant>(runner));
}

NOINLINE static void create_rgate() {
    struct SyscallRGateRunner : public Runner {
        void run() override {
            Syscalls::create_rgate(selector, 10, 10);
        }
        void post() override {
            Syscalls::revoke(Activity::own().sel(),
                             KIF::CapRngDesc(KIF::CapRngDesc::OBJ, selector, 1), true);
        }
    };

    Profile pr;
    SyscallRGateRunner runner;
    WVPERF(__func__, pr.runner<CycleInstant>(runner));
}

NOINLINE static void create_sgate() {
    struct SyscallSGateRunner : public Runner {
        explicit SyscallSGateRunner() : rgate(RecvGate::create(10, 10)) {
        }
        void run() override {
            Syscalls::create_sgate(selector, rgate.sel(), 0x1234, 1024);
        }
        void post() override {
            Syscalls::revoke(Activity::own().sel(),
                             KIF::CapRngDesc(KIF::CapRngDesc::OBJ, selector, 1), true);
        }

        RecvGate rgate;
    };

    Profile pr;
    SyscallSGateRunner runner;
    WVPERF(__func__, pr.runner<CycleInstant>(runner));
}

NOINLINE static void create_map() {
    if(!Activity::own().tile_desc().has_virtmem()) {
        println("Tile has no virtual memory support; skipping"_cf);
        return;
    }

    constexpr capsel_t DEST = 0x3000'0000 >> PAGE_BITS;

    struct SyscallMapRunner : public Runner {
        explicit SyscallMapRunner() : mgate(MemGate::create_global(PAGE_SIZE * 2, MemGate::RW)) {
        }

        void pre() override {
            // one warmup run, because the revoke leads to an unmap, which flushes and invalidates
            // all cache lines
            Syscalls::create_map(DEST, Activity::own().sel(), mgate.sel(), 0, 1, MemGate::RW);
        }

        void run() override {
            Syscalls::create_map(DEST + 1, Activity::own().sel(), mgate.sel(), 1, 1, MemGate::RW);
        }
        void post() override {
            Syscalls::revoke(Activity::own().sel(), KIF::CapRngDesc(KIF::CapRngDesc::MAP, DEST, 2),
                             true);
        }

        MemGate mgate;
    };

    Profile pr;
    SyscallMapRunner runner;
    WVPERF(__func__, pr.runner<CycleInstant>(runner));
}

NOINLINE static void create_srv() {
    struct SyscallSrvRunner : public Runner {
        explicit SyscallSrvRunner() : rgate(RecvGate::create(10, 10)) {
            rgate.activate();
        }

        void run() override {
            Syscalls::create_srv(selector, rgate.sel(), "test", 0);
        }
        void post() override {
            Syscalls::revoke(Activity::own().sel(),
                             KIF::CapRngDesc(KIF::CapRngDesc::OBJ, selector, 1), true);
        }

        RecvGate rgate;
    };

    Profile pr;
    SyscallSrvRunner runner;
    WVPERF(__func__, pr.runner<CycleInstant>(runner));
}

NOINLINE static void derive_mem() {
    struct SyscallDeriveRunner : public Runner {
        explicit SyscallDeriveRunner() : mgate(MemGate::create_global(0x1000, MemGate::RW)) {
        }

        void run() override {
            Syscalls::derive_mem(Activity::own().sel(), selector, mgate.sel(), 0, 0x1000,
                                 MemGate::RW);
        }
        void post() override {
            Syscalls::revoke(Activity::own().sel(),
                             KIF::CapRngDesc(KIF::CapRngDesc::OBJ, selector, 1), true);
        }

        MemGate mgate;
    };

    Profile pr;
    SyscallDeriveRunner runner;
    WVPERF(__func__, pr.runner<CycleInstant>(runner));
}

NOINLINE static void exchange() {
    struct SyscallExchangeRunner : public Runner {
        explicit SyscallExchangeRunner() : tile(Tile::get("own|core")), act(tile, "test") {
        }

        void run() override {
            Syscalls::exchange(act.sel(), KIF::CapRngDesc(KIF::CapRngDesc::OBJ, KIF::SEL_ACT, 1),
                               selector, false);
        }
        void post() override {
            Syscalls::revoke(act.sel(), KIF::CapRngDesc(KIF::CapRngDesc::OBJ, selector, 1), true);
        }

        Reference<Tile> tile;
        ChildActivity act;
    };

    Profile pr;
    SyscallExchangeRunner runner;
    WVPERF(__func__, pr.runner<CycleInstant>(runner));
}

NOINLINE static void revoke() {
    struct SyscallRevokeRunner : public Runner {
        void pre() override {
            mgate = new MemGate(MemGate::create_global(0x1000, MemGate::RW));
        }
        void run() override {
            delete mgate;
            mgate = nullptr;
        }

        MemGate *mgate;
    };

    Profile pr;
    SyscallRevokeRunner runner;
    WVPERF(__func__, pr.runner<CycleInstant>(runner));
}

void bsyscall() {
    selector = SelSpace::get().alloc_sel();

    RUN_BENCH(noop);
    RUN_BENCH(activate);
    RUN_BENCH(create_mgate);
    RUN_BENCH(create_rgate);
    RUN_BENCH(create_sgate);
    RUN_BENCH(create_map);
    RUN_BENCH(create_srv);
    RUN_BENCH(derive_mem);
    RUN_BENCH(exchange);
    RUN_BENCH(revoke);
}
