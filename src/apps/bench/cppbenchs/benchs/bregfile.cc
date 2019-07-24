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

#include <m3/vfs/FileRef.h>
#include <m3/Test.h>

#include "../cppbenchs.h"

using namespace m3;

alignas(64) static char buf[8192];

NOINLINE static void open_close() {
    Profile pr(20, 5);

    WVPERF("w/  file session", pr.run_with_id([] {
        FileRef file("/data/2048k.txt", FILE_R);
    }, 0x30));

    // pass one EP caps to m3fs (required for FILE_NOSESS)
    epid_t ep = VPE::self().alloc_ep();
    VFS::delegate_eps("/", VPE::self().ep_to_sel(ep), 1);

    WVPERF("w/o file session", pr.run_with_id([] {
        FileRef file("/data/2048k.txt", FILE_R | m3::FILE_NOSESS);
    }, 0x31));
}

NOINLINE static void stat() {
    Profile pr(20, 5);

    WVPERF(__func__, pr.run_with_id([] {
        FileInfo info;
        VFS::stat("/data/2048k.txt", info);
    }, 0x32));
}

NOINLINE static void mkdir_rmdir() {
    Profile pr(20, 5);

    WVPERF(__func__, pr.run_with_id([] {
        VFS::mkdir("/newdir", 0755);
        VFS::rmdir("/newdir");
    }, 0x33));
}

NOINLINE static void link_unlink() {
    Profile pr(20, 5);

    WVPERF(__func__, pr.run_with_id([] {
        VFS::link("/large.txt", "/newlarge.txt");
        VFS::unlink("/newlarge.txt");
    }, 0x34));
}

NOINLINE static void read() {
    Profile pr(2, 1);

    WVPERF("2 MiB file with 8K buf", pr.run_with_id([] {
        FileRef file("/data/2048k.txt", FILE_R);

        size_t amount;
        while((amount = file->read(buf, sizeof(buf))) > 0)
            ;
    }, 0x35));
}

NOINLINE static void write() {
    const size_t SIZE = 2 * 1024 * 1024;
    Profile pr(2, 1);

    WVPERF("2 MiB file with 8K buf", pr.run_with_id([] {
        FileRef file("/newfile", FILE_W | FILE_TRUNC | FILE_CREATE);

        size_t total = 0;
        while(total < SIZE) {
            size_t amount = file->write(buf, sizeof(buf));
            total += static_cast<size_t>(amount);
        }
    }, 0x36));
}

NOINLINE static void copy() {
    Profile pr(2, 1);

    WVPERF("2 MiB file with 8K buf", pr.run_with_id([] {
        FileRef in("/data/2048k.txt", FILE_R);
        FileRef out("/newfile", FILE_W | FILE_TRUNC | FILE_CREATE);

        size_t count;
        while((count = in->read(buf, sizeof(buf))) > 0)
            out->write_all(buf, static_cast<size_t>(count));
    }, 0x37));
}

void bregfile() {
    RUN_BENCH(open_close);
    RUN_BENCH(stat);
    RUN_BENCH(mkdir_rmdir);
    RUN_BENCH(link_unlink);
    RUN_BENCH(read);
    RUN_BENCH(write);
    RUN_BENCH(copy);
}
