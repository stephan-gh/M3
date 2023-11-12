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

#include <base/KIF.h>
#include <base/time/Instant.h>

#include <m3/Syscalls.h>
#include <m3/stream/Standard.h>
#include <m3/tiles/Activity.h>

using namespace m3;

static const size_t COUNT = 9;
static const size_t PAGES = 16;

int main() {
    if(!Activity::own().tile_desc().has_virtmem())
        exitmsg("Tile has no virtual memory support"_cf);

    const uintptr_t virt = 0x3000'0000;

    MemCap mgate = MemCap::create_global(PAGES * PAGE_SIZE, MemCap::RW);

    CycleDuration xfer;
    for(size_t i = 0; i < COUNT; ++i) {
        Syscalls::create_map(virt / PAGE_SIZE, Activity::own().sel(), mgate.sel(), 0, PAGES,
                             MemCap::RW);

        MemGate mapped_mem =
            Activity::own().get_mem(virt, PAGES * PAGE_SIZE, MemGate::R).activate();

        alignas(8) char buf[8];
        for(size_t p = 0; p < PAGES; ++p) {
            auto start = CycleInstant::now();
            mapped_mem.read(buf, sizeof(buf), p * PAGE_SIZE);
            auto end = CycleInstant::now();
            xfer += end.duration_since(start);
        }

        Syscalls::revoke(Activity::own().sel(),
                         KIF::CapRngDesc(KIF::CapRngDesc::MAP, virt / PAGE_SIZE, PAGES), true);
    }

    println("per-xfer: {}"_cf, xfer / (COUNT * PAGES));
    return 0;
}
