/*
 * Copyright (C) 2016-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/time/Instant.h>

#include <m3/stream/Standard.h>
#include <m3/tiles/ChildActivity.h>
#include <m3/vfs/MountTable.h>

using namespace m3;

int main(int argc, char **argv) {
    if(argc < 2)
        exitmsg("Usage: " << argv[0] << " <file>...");

    int res;
    auto start = TimeInstant::now();
    {
        auto tile = Tile::get("own|core");
        ChildActivity child(tile, argv[1]);
        child.add_file(STDIN_FD, STDIN_FD);
        child.add_file(STDOUT_FD, STDOUT_FD);
        child.add_file(STDERR_FD, STDERR_FD);
        child.add_mount("/", "/");

        child.exec(argc - 1, const_cast<const char**>(argv) + 1);

        res = child.wait();
    }

    auto end = TimeInstant::now();

    cerr << "Activity (" << argv[1] << ") terminated with exit-code " << res << "\n";
    cerr << "Runtime: " << end.duration_since(start) << "\n";
    return 0;
}
