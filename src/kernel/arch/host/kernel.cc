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
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/un.h>
#include <cstdio>
#include <cstdlib>
#include <fcntl.h>
#include <dirent.h>
#include <pthread.h>
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

static sockaddr_un get_sock(const char *name) {
    sockaddr_un addr;
    memset(&addr, 0, sizeof(addr));
    addr.sun_family = AF_UNIX;
    // we can't put that in the format string
    addr.sun_path[0] = '\0';
    snprintf(addr.sun_path + 1, sizeof(addr.sun_path) - 1, "m3_net_%s", name);
    return addr;
}

class Bridge {
public:
    explicit Bridge(const std::string &from, const std::string &to)
        : _name(from + " -> " + to),
          _src_fd(),
          _dst_fd(),
          _dst_sock() {
        _src_fd = socket(AF_UNIX, SOCK_DGRAM, 0);
        if(_src_fd == -1)
            PANIC("Unable to create socket for " << from.c_str() << ": " << strerror(errno));
        _dst_fd = socket(AF_UNIX, SOCK_DGRAM, 0);
        if(_dst_fd == -1)
            PANIC("Unable to create socket for " << to.c_str() << ": " << strerror(errno));

        _dst_sock = get_sock(to.c_str());

        sockaddr_un src_sock = get_sock(from.c_str());
        if(bind(_src_fd, (struct sockaddr*)&src_sock, sizeof(src_sock)) == -1)
            PANIC("Binding socket for " << from.c_str() << "-in failed: " << strerror(errno));
    }
    ~Bridge() {
        close(_dst_fd);
        close(_src_fd);
    }

    void check() {
        char buffer[2048];
        ssize_t res = recvfrom(_src_fd, buffer, sizeof(buffer), MSG_DONTWAIT, nullptr, nullptr);
        if(res <= 0)
            return;

        if(sendto(_dst_fd, buffer, static_cast<size_t>(res), 0,
                  (struct sockaddr*)&_dst_sock, sizeof(_dst_sock)) == -1)
            KLOG(ERR, "Unable to forward packet: " << strerror(errno));
    }

private:
    std::string _name;
    int _src_fd;
    int _dst_fd;
    sockaddr_un _dst_sock;
};

static void *bridge_thread(void *arg) {
    std::string *bridge = reinterpret_cast<std::string*>(arg);

    size_t split = bridge->find("-");
    std::string src_name = bridge->substr(0, split);
    std::string dst_name = bridge->substr(split + 1);

    Bridge b1(src_name + "_out", dst_name + "_in");
    Bridge b2(dst_name + "_out", src_name + "_in");

    while(1) {
        b1.check();
        b2.check();
    }
    return nullptr;
}

static void create_bridge(const char *bridge) {
    pthread_t tid;
    int res = pthread_create(&tid, nullptr, bridge_thread, new std::string(bridge));
    if(res == -1)
        PANIC("Unable to create bridge thread");
}

int main(int argc, char *argv[]) {
    int argstart = Args::parse(argc, argv);

    mkdir("/tmp/m3", 0755);
    signal(SIGINT, sigint);

    if(Args::bridge)
        create_bridge(Args::bridge);

    MainMemory::init();
    KLOG(MEM, MainMemory::get());

    WorkLoop &wl = WorkLoop::get();

    // create some worker threads
    wl.multithreaded(8);

    Platform::init();
    Platform::add_modules(argc - argstart, argv + argstart);
    if(Args::fsimg)
        copyfromfs(MainMemory::get(), Args::fsimg);
    SyscallHandler::init();
    PEManager::create();
    VPEManager::create();
    VPEManager::get().start_root();

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
