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

#include <base/time/Instant.h>
#include <base/KIF.h>

#include <m3/stream/Standard.h>
#include <m3/Syscalls.h>
#include <m3/tiles/Activity.h>

using namespace m3;

static const size_t COUNT       = 9;
static const size_t PAGES       = 16;

int main() {
    if(!Activity::self().tile_desc().has_virtmem())
        exitmsg("Tile has no virtual memory support");

    const uintptr_t virt = 0x30000000;

    MemGate mgate = MemGate::create_global(PAGES * PAGE_SIZE, MemGate::RW);

    CycleDuration xfer;
    for(size_t i = 0; i < COUNT; ++i) {
        Syscalls::create_map(
            virt / PAGE_SIZE, Activity::self().sel(), mgate.sel(), 0, PAGES, MemGate::RW
        );

        MemGate mapped_mem = Activity::self().get_mem(virt, PAGES * PAGE_SIZE, MemGate::R);

        alignas(8) char buf[8];
        for(size_t p = 0; p < PAGES; ++p) {
            auto start = CycleInstant::now();
            mapped_mem.read(buf, sizeof(buf), p * PAGE_SIZE);
            auto end = CycleInstant::now();
            xfer += end.duration_since(start);
        }

        Syscalls::revoke(
            Activity::self().sel(), KIF::CapRngDesc(KIF::CapRngDesc::MAP, virt / PAGE_SIZE, PAGES), true
        );
    }

    cout << "per-xfer: " << (xfer / (COUNT * PAGES)) << "\n";
    return 0;
}
