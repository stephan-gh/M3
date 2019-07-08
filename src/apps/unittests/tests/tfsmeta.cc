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

#include <base/stream/IStringStream.h>

#include <m3/stream/FStream.h>
#include <m3/vfs/Dir.h>
#include <m3/vfs/FileRef.h>
#include <m3/vfs/VFS.h>

#include <algorithm>
#include <vector>

#include "../unittests.h"

using namespace m3;

static void dir_listing() {
    // read a dir with known content
    const char *dirname = "/largedir";
    Dir dir(dirname);

    Dir::Entry e;
    std::vector<Dir::Entry> entries;
    while(dir.readdir(e))
        entries.push_back(e);
    assert_size(entries.size(), 82);

    // we don't know the order because it is determined by the host OS. thus, sort it first.
    std::sort(entries.begin(), entries.end(), [] (const Dir::Entry &a, const Dir::Entry &b) -> bool {
        bool aspec = strcmp(a.name, ".") == 0 || strcmp(a.name, "..") == 0;
        bool bspec = strcmp(b.name, ".") == 0 || strcmp(b.name, "..") == 0;
        if(aspec && bspec)
            return strcmp(a.name, b.name) < 0;
        if(aspec)
            return true;
        if(bspec)
            return false;
        return IStringStream::read_from<int>(a.name) < IStringStream::read_from<int>(b.name);
    });

    // now check file names
    assert_str(entries[0].name, ".");
    assert_str(entries[1].name, "..");
    for(size_t i = 0; i < 80; ++i) {
        char tmp[16];
        OStringStream os(tmp, sizeof(tmp));
        os << i << ".txt";
        assert_str(entries[i + 2].name, os.str());
    }
}

static void meta_operations() {
    VFS::mkdir("/example", 0755);
    assert_err(Errors::EXISTS, [] { VFS::mkdir("/example", 0755); });
    assert_err(Errors::NO_SUCH_FILE, [] { VFS::mkdir("/example/foo/bar", 0755); });

    {
        FStream f("/example/myfile", FILE_W | FILE_CREATE);
        f << "test\n";
    }

    {
        VFS::mount("/fs/", "m3fs", "m3fs-clone");
        assert_err(Errors::XFS_LINK, [] { VFS::link("/example/myfile", "/fs/foo"); });
        VFS::unmount("/fs");
    }

    assert_err(Errors::NO_SUCH_FILE, [] { VFS::rmdir("/example/foo/bar"); });
    assert_err(Errors::IS_NO_DIR, [] { VFS::rmdir("/example/myfile"); });
    assert_err(Errors::DIR_NOT_EMPTY, [] { VFS::rmdir("/example"); });

    assert_err(Errors::IS_DIR, [] { VFS::link("/example", "/newpath"); });
    VFS::link("/example/myfile", "/newpath");

    assert_err(Errors::IS_DIR, [] { VFS::unlink("/example"); });
    assert_err(Errors::NO_SUCH_FILE, [] { VFS::unlink("/example/foo"); });
    VFS::unlink("/example/myfile");

    VFS::rmdir("/example");
    VFS::unlink("/newpath");
}

static void delete_file() {
    const char *tmp_file = "/tmp_file.txt";

    {
        FStream f(tmp_file, FILE_W | FILE_CREATE);
        f << "test\n";
    }

    {
        char buffer[32];

        FileRef file(tmp_file, FILE_R);

        VFS::unlink(tmp_file);

        assert_err(Errors::NO_SUCH_FILE, [&tmp_file] { VFS::open(tmp_file, FILE_R); });

        assert_size(file->read(buffer, sizeof(buffer)), 5);
    }

    assert_err(Errors::NO_SUCH_FILE, [&tmp_file] { VFS::open(tmp_file, FILE_R); });
}

void tfsmeta() {
    RUN_TEST(dir_listing);
    RUN_TEST(meta_operations);
    RUN_TEST(delete_file);
}
