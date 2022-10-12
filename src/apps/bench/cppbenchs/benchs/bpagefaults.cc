/*
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
#include <m3/session/Pager.h>
#include <m3/vfs/VFS.h>

#include "../cppbenchs.h"

using namespace m3;

static const size_t PAGES = 64;

NOINLINE static void anon() {
    Profile pr(4, 4);
    WVPERF("anon mapping (64 pages)", pr.run<CycleInstant>([] {
        goff_t virt = 0x3000'0000;
        Activity::own().pager()->map_anon(&virt, PAGES * PAGE_SIZE, Pager::READ | Pager::WRITE, 0);

        auto data = reinterpret_cast<char *>(virt);
        for(size_t i = 0; i < PAGES; ++i)
            data[i * PAGE_SIZE] = i;

        Activity::own().pager()->unmap(virt);
    }));
}

NOINLINE static void file() {
    Profile pr(4, 4);
    WVPERF("file mapping (64 pages)", pr.run<CycleInstant>([] {
        auto file = VFS::open("/large.bin", FILE_RW | FILE_NEWSESS);

        goff_t virt = 0x3100'0000;
        file->map(Activity::own().pager(), &virt, 0, PAGES * PAGE_SIZE, Pager::READ | Pager::WRITE,
                  0);

        auto data = reinterpret_cast<char *>(virt);
        for(size_t i = 0; i < PAGES; ++i)
            data[i * PAGE_SIZE] = i;

        Activity::own().pager()->unmap(virt);
    }));
}

void bpagefaults() {
    if(!Activity::own().tile_desc().has_virtmem()) {
        println("Tile has no virtual memory support; skipping pagefault benchmark."_cf);
        return;
    }

    RUN_BENCH(anon);
    RUN_BENCH(file);
}
