/*
 * Copyright (C) 2016-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <m3/stream/Standard.h>
#include <m3/pes/VPE.h>
#include <m3/vfs/MountTable.h>

using namespace m3;

int main(int argc, char **argv) {
    if(argc < 2)
        exitmsg("Usage: " << argv[0] << " <file>...");

    int res;
    auto start = TimeInstant::now();
    {
        auto pe = PE::get("own|core");
        VPE child(pe, argv[1]);
        child.files()->set(STDIN_FD, VPE::self().files()->get(STDIN_FD));
        child.files()->set(STDOUT_FD, VPE::self().files()->get(STDOUT_FD));
        child.files()->set(STDERR_FD, VPE::self().files()->get(STDERR_FD));
        child.mounts()->add("/", VPE::self().mounts()->get("/"));

        child.exec(argc - 1, const_cast<const char**>(argv) + 1);

        res = child.wait();
    }

    auto end = TimeInstant::now();

    cerr << "VPE (" << argv[1] << ") terminated with exit-code " << res << "\n";
    cerr << "Runtime: " << end.duration_since(start) << "\n";
    return 0;
}
