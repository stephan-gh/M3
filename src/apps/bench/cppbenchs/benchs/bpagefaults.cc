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
#include <base/time/Profile.h>
#include <base/Panic.h>

#include <m3/vfs/FileRef.h>
#include <m3/session/Pager.h>
#include <m3/Test.h>

#include "../cppbenchs.h"

using namespace m3;

static const size_t PAGES = 64;

NOINLINE static void anon() {
    Profile pr(4, 4);
    WVPERF("anon mapping (64 pages)", pr.run<CycleInstant>([] {
        goff_t virt = 0x30000000;
        Activity::self().pager()->map_anon(&virt, PAGES * PAGE_SIZE, Pager::READ | Pager::WRITE, 0);

        auto data = reinterpret_cast<char*>(virt);
        for(size_t i = 0; i < PAGES; ++i)
            data[i * PAGE_SIZE] = i;

        Activity::self().pager()->unmap(virt);
    }));
}

NOINLINE static void file() {
    Profile pr(4, 4);
    WVPERF("file mapping (64 pages)", pr.run<CycleInstant>([] {
        FileRef f("/large.bin", FILE_RW | FILE_NEWSESS);

        goff_t virt = 0x31000000;
        f->map(Activity::self().pager(), &virt, 0, PAGES * PAGE_SIZE, Pager::READ | Pager::WRITE, 0);

        auto data = reinterpret_cast<char*>(virt);
        for(size_t i = 0; i < PAGES; ++i)
            data[i * PAGE_SIZE] = i;

        Activity::self().pager()->unmap(virt);
    }));
}

void bpagefaults() {
    if(!Activity::self().tile_desc().has_virtmem()) {
        cout << "Tile has no virtual memory support; skipping pagefault benchmark.\n";
        return;
    }

    RUN_BENCH(anon);
    RUN_BENCH(file);
}
