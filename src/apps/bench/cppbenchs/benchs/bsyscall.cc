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
#include <base/col/SList.h>
#include <base/util/Profile.h>
#include <base/KIF.h>
#include <base/Panic.h>

#include <m3/Syscalls.h>
#include <m3/Test.h>

#include "../cppbenchs.h"

using namespace m3;

static capsel_t selector = ObjCap::INVALID;

NOINLINE static void noop() {
    Profile pr;
    WVPERF(__func__, pr.run_with_id([] {
        Syscalls::noop();
    }, 0x50));
}

NOINLINE static void activate() {
    EP ep = EP::alloc();
    MemGate mgate = MemGate::create_global(0x1000, MemGate::RW);

    Profile pr;
    WVPERF(__func__, pr.run_with_id([&ep, &mgate] {
        Syscalls::activate(ep.sel(), mgate.sel(), 0);
    }, 0x51));
}

NOINLINE static void create_rgate() {
    struct SyscallRGateRunner : public Runner {
        void run() override {
            Syscalls::create_rgate(selector, 10, 10);
        }
        void post() override {
            Syscalls::revoke(VPE::self().sel(),
                KIF::CapRngDesc(KIF::CapRngDesc::OBJ, selector, 1), true);
        }
    };

    Profile pr;
    SyscallRGateRunner runner;
    WVPERF(__func__, pr.runner_with_id(runner, 0x52));
}

NOINLINE static void create_sgate() {
    struct SyscallSGateRunner : public Runner {
        explicit SyscallSGateRunner() : rgate(RecvGate::create(10, 10)) {
        }
        void run() override {
            Syscalls::create_sgate(selector, rgate.sel(), 0x1234, 1024);
        }
        void post() override {
            Syscalls::revoke(VPE::self().sel(),
                KIF::CapRngDesc(KIF::CapRngDesc::OBJ, selector, 1), true);
        }

        RecvGate rgate;
    };

    Profile pr;
    SyscallSGateRunner runner;
    WVPERF(__func__, pr.runner_with_id(runner, 0x53));
}

NOINLINE static void create_map() {
    if(!VPE::self().pe_desc().has_virtmem()) {
        cout << "PE has no virtual memory support; skipping\n";
        return;
    }

    constexpr capsel_t DEST = 0x30000000 >> PAGE_BITS;

    struct SyscallMapRunner : public Runner {
        explicit SyscallMapRunner() : mgate(MemGate::create_global(0x1000, MemGate::RW)) {
        }

        void run() override {
            Syscalls::create_map(DEST, 0, mgate.sel(), 0, 1, MemGate::RW);
        }
        void post() override {
            Syscalls::revoke(VPE::self().sel(),
                KIF::CapRngDesc(KIF::CapRngDesc::MAP, DEST, 1), true);
        }

        MemGate mgate;
    };

    Profile pr;
    SyscallMapRunner runner;
    WVPERF(__func__, pr.runner_with_id(runner, 0x55));
}

NOINLINE static void create_srv() {
    struct SyscallSrvRunner : public Runner {
        explicit SyscallSrvRunner() : rgate(RecvGate::create(10, 10)) {
            rgate.activate();
        }

        void run() override {
            Syscalls::create_srv(selector, VPE::self().sel(), rgate.sel(), "test");
        }
        void post() override {
            Syscalls::revoke(VPE::self().sel(),
                KIF::CapRngDesc(KIF::CapRngDesc::OBJ, selector, 1), true);
        }

        RecvGate rgate;
    };

    Profile pr;
    SyscallSrvRunner runner;
    WVPERF(__func__, pr.runner_with_id(runner, 0x56));
}

NOINLINE static void derive_mem() {
    struct SyscallDeriveRunner : public Runner {
        explicit SyscallDeriveRunner() : mgate(MemGate::create_global(0x1000, MemGate::RW)) {
        }

        void run() override {
            Syscalls::derive_mem(VPE::self().sel(), selector, mgate.sel(), 0, 0x1000, MemGate::RW);
        }
        void post() override {
            Syscalls::revoke(VPE::self().sel(),
                KIF::CapRngDesc(KIF::CapRngDesc::OBJ, selector, 1), true);
        }

        MemGate mgate;
    };

    Profile pr;
    SyscallDeriveRunner runner;
    WVPERF(__func__, pr.runner_with_id(runner, 0x58));
}

NOINLINE static void exchange() {
    struct SyscallExchangeRunner : public Runner {
        explicit SyscallExchangeRunner()
            : pe(PE::alloc(VPE::self().pe_desc())),
              vpe(pe, "test") {
        }

        void run() override {
            Syscalls::exchange(vpe.sel(),
                KIF::CapRngDesc(KIF::CapRngDesc::OBJ, KIF::SEL_MEM, 1), selector, false);
        }
        void post() override {
            Syscalls::revoke(vpe.sel(), KIF::CapRngDesc(KIF::CapRngDesc::OBJ, selector, 1), true);
        }

        PE pe;
        VPE vpe;
    };

    Profile pr;
    SyscallExchangeRunner runner;
    WVPERF(__func__, pr.runner_with_id(runner, 0x59));
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
    WVPERF(__func__, pr.runner_with_id(runner, 0x5A));
}

void bsyscall() {
    selector = VPE::self().alloc_sel();

    RUN_BENCH(noop);
    RUN_BENCH(activate);
    RUN_BENCH(create_rgate);
    RUN_BENCH(create_sgate);
    RUN_BENCH(create_map);
    RUN_BENCH(create_srv);
    RUN_BENCH(derive_mem);
    RUN_BENCH(exchange);
    RUN_BENCH(revoke);
}
