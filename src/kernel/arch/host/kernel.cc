/*
 * Copyright (C) 2015-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/col/SList.h>
#include <base/log/Kernel.h>
#include <base/Config.h>
#include <base/DTU.h>
#include <base/Panic.h>

#include <sys/types.h>
#include <sys/wait.h>
#include <sys/time.h>
#include <sys/stat.h>
#include <cstdio>
#include <cstdlib>
#include <fcntl.h>
#include <dirent.h>
#include <unistd.h>

#include "pes/PEManager.h"
#include "pes/VPEManager.h"
#include "pes/VPE.h"
#include "Args.h"
#include "SyscallHandler.h"
#include "WorkLoop.h"

using namespace kernel;

static size_t fssize = 0;

static void sigint(int) {
    WorkLoop::get().stop();
}

static void delete_dir(const char *dir) {
    char path[64];
    DIR *d = opendir(dir);
    struct dirent *e;
    while((e = readdir(d))) {
        if(strcmp(e->d_name, ".") == 0 || strcmp(e->d_name, "..") == 0)
            continue;
        snprintf(path, sizeof(path), "%s/%s", dir, e->d_name);
        unlink(path);
    }
    closedir(d);
    rmdir(dir);
}

static void copyfromfs(MainMemory &mem, const char *file) {
    int fd = open(file, O_RDONLY);
    if(fd < 0)
        PANIC("Opening '" << file << "' for reading failed");

    struct stat info;
    if(fstat(fd, &info) == -1)
        PANIC("Stat for '" << file << "' failed");
    if(info.st_size > FS_MAX_SIZE) {
        PANIC("Filesystem image (" << file << ") too large"
         " (max=" << FS_MAX_SIZE << ", size=" << info.st_size << ")");
    }

    goff_t fs_addr = mem.module(0).addr() + FS_IMG_OFFSET;
    ssize_t res = read(fd, reinterpret_cast<void*>(fs_addr), FS_MAX_SIZE);
    if(res == -1)
        PANIC("Reading from '" << file << "' failed");
    close(fd);

    fssize = static_cast<size_t>(res);
    KLOG(MEM, "Copied fs-image '" << file << "' to 0.." << m3::fmt(fssize, "#x"));
}

static void copytofs(MainMemory &mem, const char *file) {
    char name[256];
    snprintf(name, sizeof(name), "%s.out", file);
    int fd = open(name, O_WRONLY | O_TRUNC | O_CREAT, 0644);
    if(fd < 0)
        PANIC("Opening '" << name << "' for writing failed");

    goff_t fs_addr = mem.module(0).addr() + FS_IMG_OFFSET;
    write(fd, reinterpret_cast<void*>(fs_addr), fssize);
    close(fd);

    KLOG(MEM, "Copied fs-image from memory back to '" << name << "'");
}

int main(int argc, char *argv[]) {
    int argstart = Args::parse(argc, argv);

    mkdir("/tmp/m3", 0755);
    signal(SIGINT, sigint);

    KLOG(MEM, MainMemory::get());

    WorkLoop &wl = WorkLoop::get();

    // create some worker threads
    wl.multithreaded(8);

    Platform::add_modules(argc - argstart - 1, argv + argstart + 1);
    if(Args::fsimg)
        copyfromfs(MainMemory::get(), Args::fsimg);
    SyscallHandler::init();
    PEManager::create();
    VPEManager::create();
    VPEManager::get().start_root();
    PEManager::get().init();

    KLOG(INFO, "Kernel is ready");

    wl.run();

    KLOG(INFO, "Shutting down");
    if(Args::fsimg)
        copytofs(MainMemory::get(), Args::fsimg);
    VPEManager::destroy();
    delete_dir("/tmp/m3");

    size_t blocked = m3::ThreadManager::get().blocked_count();
    if(blocked > 0)
        KLOG(ERR, "\e[37;41m" << blocked << " blocked threads left\e[0m");

    return EXIT_SUCCESS;
}
