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

#include <m3/stream/Standard.h>

#include <m3/vfs/FileRef.h>

#include "../cppbenchs.h"

using namespace m3;

alignas(64) static char buf[8192];

NOINLINE static void open_close() {
    Profile pr(20, 5);

    cout << "w/  file session: " << pr.run_with_id([] {
        FileRef file("/data/2048k.txt", FILE_R);
        if(Errors::occurred())
            PANIC("Unable to open file '/data/2048k.txt'");
    }, 0x30) << "\n";

    // pass one EP caps to m3fs (required for FILE_NOSESS)
    epid_t ep = VPE::self().alloc_ep();
    if(ep == EP_COUNT)
        PANIC("Unable to allocate EP for meta session");
    if(VFS::delegate_eps("/", VPE::self().ep_to_sel(ep), 1) != Errors::NONE)
        PANIC("Unable to delegate EPs to meta session");

    cout << "w/o file session: " << pr.run_with_id([] {
        FileRef file("/data/2048k.txt", FILE_R | m3::FILE_NOSESS);
        if(Errors::occurred())
            PANIC("Unable to open file '/data/2048k.txt'");
    }, 0x31) << "\n";
}

NOINLINE static void stat() {
    Profile pr(20, 5);

    cout << pr.run_with_id([] {
        FileInfo info;
        if(VFS::stat("/data/2048k.txt", info) != Errors::NONE)
            PANIC("Unable to stat file '/data/2048k.txt'");
    }, 0x32) << "\n";
}

NOINLINE static void mkdir_rmdir() {
    Profile pr(20, 5);

    cout << pr.run_with_id([] {
        if(VFS::mkdir("/newdir", 0755) != Errors::NONE)
            PANIC("Unable to mkdir '/newdir'");
        if(VFS::rmdir("/newdir") != Errors::NONE)
            PANIC("Unable to rmdir '/newdir'");
    }, 0x33) << "\n";
}

NOINLINE static void link_unlink() {
    Profile pr(20, 5);

    cout << pr.run_with_id([] {
        if(VFS::link("/large.txt", "/newlarge.txt") != Errors::NONE)
            PANIC("Unable to link '/newlarge.txt' to '/large.txt'");
        if(VFS::unlink("/newlarge.txt") != Errors::NONE)
            PANIC("Unable to unlink '/newlarge.txt'");
    }, 0x34) << "\n";
}

NOINLINE static void read() {
    Profile pr(2, 1);

    cout << "2 MiB file with 8K buf: " << pr.run_with_id([] {
        FileRef file("/data/2048k.txt", FILE_R);
        if(Errors::occurred())
            PANIC("Unable to open file '/data/2048k.txt'");

        ssize_t amount;
        while((amount = file->read(buf, sizeof(buf))) > 0)
            ;
    }, 0x35) << "\n";
}

NOINLINE static void write() {
    const size_t SIZE = 2 * 1024 * 1024;
    Profile pr(2, 1);

    cout << "2 MiB file with 8K buf: " << pr.run_with_id([] {
        FileRef file("/newfile", FILE_W | FILE_TRUNC | FILE_CREATE);
        if(Errors::occurred())
            PANIC("Unable to open file '/newfile'");

        size_t total = 0;
        while(total < SIZE) {
            ssize_t amount = file->write(buf, sizeof(buf));
            if(amount <= 0)
                PANIC("Unable to write to file");
            total += static_cast<size_t>(amount);
        }
    }, 0x36) << "\n";
}

NOINLINE static void copy() {
    Profile pr(2, 1);

    cout << "2 MiB file with 8K buf: " << pr.run_with_id([] {
        FileRef in("/data/2048k.txt", FILE_R);
        if(Errors::occurred())
            PANIC("Unable to open file '/data/2048k.txt'");

        FileRef out("/newfile", FILE_W | FILE_TRUNC | FILE_CREATE);
        if(Errors::occurred())
            PANIC("Unable to open file '/newfile'");

        ssize_t count;
        while((count = in->read(buf, sizeof(buf))) > 0)
            out->write_all(buf, static_cast<size_t>(count));
    }, 0x37) << "\n";
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
