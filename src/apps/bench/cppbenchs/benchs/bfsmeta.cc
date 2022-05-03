/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2021 Nils Asmussen, Barkhausen Institut
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
#include <m3/vfs/VFS.h>

#include "../cppbenchs.h"

using namespace m3;

NOINLINE static void stat() {
    Profile pr(50, 20);

    WVPERF("Stat in root dir", pr.run<CycleInstant>([] {
        FileInfo info;
        VFS::stat("/large.txt", info);
    }));

    WVPERF("Stat in sub dir", pr.run<CycleInstant>([] {
        FileInfo info;
        VFS::stat("/finddata/dir/dir-1/32.txt", info);
    }));
}

void bfsmeta() {
    RUN_BENCH(stat);
}
