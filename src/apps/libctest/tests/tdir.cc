/*
 * Copyright (C) 2021 Nils Asmussen, Barkhausen Institut
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

#include <m3/Test.h>

#include <algorithm>
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string>
#include <sys/stat.h>
#include <unistd.h>
#include <vector>

#include "../libctest.h"

using namespace m3;

static void mkdir_rmdir() {
    WVASSERTEQ(mkdir("/tmp/foo", 0755), 0);
    WVASSERTECODE(EEXIST, mkdir("/tmp/foo", 0755));

    {
        int fd = open("/tmp/foo/myfile.txt", O_WRONLY | O_CREAT);
        WVASSERT(fd >= 0);
        WVASSERTEQ(write(fd, "test", 4), 4);
        close(fd);
    }

    WVASSERTECODE(ENOTEMPTY, rmdir("/tmp/foo"));
    WVASSERTEQ(unlink("/tmp/foo/myfile.txt"), 0);
    WVASSERTEQ(rmdir("/tmp/foo"), 0);
    WVASSERTECODE(ENOENT, rmdir("/tmp/foo"));
}

static void rename() {
    {
        int fd = open("/tmp/myfile.txt", O_WRONLY | O_CREAT);
        WVASSERT(fd >= 0);
        close(fd);
    }

    WVASSERTEQ(rename("/tmp/myfile.txt", "/tmp/yourfile.txt"), 0);
    WVASSERTECODE(ENOENT, unlink("/tmp/myfile.txt"));
    WVASSERTEQ(unlink("/tmp/yourfile.txt"), 0);
}

static void listing() {
    DIR *d = opendir("/largedir");
    WVASSERT(d != nullptr);

    struct dirent *e;
    std::vector<int> entries;
    while((e = readdir(d))) {
        if(strcmp(e->d_name, ".") == 0 || strcmp(e->d_name, "..") == 0)
            continue;
        entries.push_back(std::stoi(e->d_name));
    }
    closedir(d);

    WVASSERTEQ(entries.size(), 80u);
    std::sort(entries.begin(), entries.end());
    for(size_t i = 0; i < 80; ++i)
        WVASSERTEQ(entries[i], static_cast<int>(i));
}

static void stat() {
    struct stat st;

    {
        WVASSERTEQ(stat("/test.txt", &st), 0);
        WVASSERTEQ(st.st_size, 15);
    }

    {
        int fd = open("/test.txt", O_RDONLY);
        WVASSERTEQ(fstat(fd, &st), 0);
        WVASSERTEQ(st.st_size, 15);
    }
}

void tdir() {
    RUN_TEST(mkdir_rmdir);
    RUN_TEST(rename);
    RUN_TEST(listing);
    RUN_TEST(stat);
}
