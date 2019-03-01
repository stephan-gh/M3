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

#include <base/ELF.h>
#include <base/Env.h>
#include <base/Panic.h>

#include <m3/session/ResMng.h>
#include <m3/stream/FStream.h>
#include <m3/vfs/FileRef.h>
#include <m3/vfs/FileTable.h>
#include <m3/vfs/MountTable.h>
#include <m3/Syscalls.h>
#include <m3/VPE.h>

#include <sys/fcntl.h>
#include <sys/stat.h>
#include <stdlib.h>
#include <unistd.h>

namespace m3 {

// this should be enough for now
static const size_t STATE_BUF_SIZE    = 4096;

static void write_file(pid_t pid, const char *suffix, const void *data, size_t size) {
    if(data) {
        char path[64];
        snprintf(path, sizeof(path), "/tmp/m3/%d-%s", pid, suffix);
        int fd = open(path, O_WRONLY | O_TRUNC | O_CREAT, 0600);
        if(fd < 0)
            perror("open");
        else {
            write(fd, data, size);
            close(fd);
        }
    }
}

static void write_file(pid_t pid, const char *suffix, uint64_t value) {
    uint8_t buf[sizeof(uint64_t)];
    Marshaller m(buf, sizeof(buf));
    m << value;
    write_file(pid, suffix, buf, m.total());
}

static void *read_from(const char *suffix, void *dst, size_t &size) {
    char path[64];
    snprintf(path, sizeof(path), "/tmp/m3/%d-%s", getpid(), suffix);
    int fd = open(path, O_RDONLY);
    if(fd >= 0) {
        if(dst == nullptr) {
            struct stat st;
            if(fstat(fd, &st) == -1)
                return nullptr;
            size = static_cast<size_t>(st.st_size);
            dst = malloc(size);
        }

        read(fd, dst, size);
        unlink(path);
        close(fd);
        return dst;
    }
    return nullptr;
}

template<typename T>
static bool read_from(const char *suffix, T *val) {
    uint8_t buf[sizeof(uint64_t)];
    size_t len = sizeof(buf);
    if(read_from(suffix, buf, len)) {
        Unmarshaller um(buf, len);
        um >> *val;
        return true;
    }
    return false;
}

static void write_state(pid_t pid, capsel_t nextsel, uint64_t eps, capsel_t rmng,
                        uint64_t rbufcur, uint64_t rbufend,
                        FileTable &files, MountTable &mounts) {
    size_t len = STATE_BUF_SIZE;
    unsigned char *buf = new unsigned char[len];

    write_file(pid, "nextsel", nextsel);
    write_file(pid, "eps", eps);
    write_file(pid, "rmng", rmng);

    Marshaller m(buf, len);
    m << rbufcur << rbufend;
    write_file(pid, "rbufs", buf, m.total());

    len = mounts.serialize(buf, STATE_BUF_SIZE);
    write_file(pid, "ms", buf, len);

    len = files.serialize(buf, STATE_BUF_SIZE);
    write_file(pid, "fds", buf, len);

    delete[] buf;
}

void VPE::init_state() {
    read_from("nextsel", &_next_sel);
    read_from("eps", &_eps);

    capsel_t rmng_sel;
    if(read_from("rmng", &rmng_sel)) {
        delete _resmng;
        _resmng = new ResMng(rmng_sel);
    }
    else if(_resmng == nullptr)
        _resmng = new ResMng(ObjCap::INVALID);

    size_t len = sizeof(uint64_t) * 2;
    uint8_t buf[len];
    if(read_from("rbufs", buf, len)) {
        Unmarshaller um(buf, len);
        um >> _rbufcur >> _rbufend;
    }
}

void VPE::init_fs() {
    if(_fds)
        delete _fds;
    if(_ms) {
        _ms->remove_all();
        delete _ms;
    }

    size_t len = STATE_BUF_SIZE;
    char *buf = new char[len];

    memset(buf, 0, len);
    if(read_from("ms", buf, len))
        _ms = MountTable::unserialize(buf, len);

    len = STATE_BUF_SIZE;
    memset(buf, 0, len);
    if(read_from("fds", buf, len))
        _fds = FileTable::unserialize(buf, len);

    delete[] buf;
}

Errors::Code VPE::run(void *lambda) {
    char byte = 1;
    int fd[2];
    if(pipe(fd) == -1)
        return Errors::OUT_OF_MEM;

    int pid = fork();
    if(pid == -1) {
        close(fd[1]);
        close(fd[0]);
        return Errors::OUT_OF_MEM;
    }
    else if(pid == 0) {
        // child
        close(fd[1]);

        // wait until parent notifies us
        read(fd[0], &byte, 1);
        close(fd[0]);

        env()->reset();
        VPE::self().init_state();
        VPE::self().init_fs();

        std::function<int()> *func = reinterpret_cast<std::function<int()>*>(lambda);
        (*func)();
        exit(0);
    }
    else {
        // parent
        close(fd[0]);

        // let the kernel create the config-file etc. for the given pid
        xfer_t arg = static_cast<xfer_t>(pid);
        Syscalls::get().vpectrl(sel(), KIF::Syscall::VCTRL_START, arg);

        write_state(pid, _next_sel, _eps, _resmng->sel(), _rbufcur, _rbufend, *_fds, *_ms);

        // notify child; it can start now
        write(fd[1], &byte, 1);
        close(fd[1]);
    }
    return Errors::NONE;
}

Errors::Code VPE::exec(int argc, const char **argv) {
    static char buffer[1024];
    char templ[] = "/tmp/m3-XXXXXX";
    int tmp, pid, fd[2];
    ssize_t res;
    char byte = 1;
    if(pipe(fd) == -1)
        return Errors::OUT_OF_MEM;

    FileRef bin(argv[0], FILE_R);
    if(Errors::occurred())
        goto errorTemp;
    tmp = mkstemp(templ);
    if(tmp < 0)
        goto errorTemp;

    // copy executable from M3-fs to a temp file
    while((res = bin->read(buffer, sizeof(buffer))) > 0)
        write(tmp, buffer, static_cast<size_t>(res));

    pid = fork();
    if(pid == -1)
        goto errorExec;
    else if(pid == 0) {
        // child
        close(fd[1]);

        // wait until parent notifies us
        read(fd[0], &byte, 1);
        close(fd[0]);

        // copy args to null-terminate them
        char **args = new char*[argc + 1];
        for(int i = 0; i < argc; ++i)
            args[i] = const_cast<char*>(argv[i]);
        args[argc] = nullptr;

        // open it readonly again as fexecve requires
        int tmpdup = open(templ, O_RDONLY);
        // we don't need it anymore afterwards
        unlink(templ);
        // it needs to be executable
        fchmod(tmpdup, 0700);
        // close writable fd to make it non-busy
        close(tmp);

        // execute that file
        fexecve(tmpdup, args, environ);
        PANIC("Exec of '" << argv[0] << "' failed: " << strerror(errno));
    }
    else {
        // parent
        close(fd[0]);
        close(tmp);

        // let the kernel create the config-file etc. for the given pid
        xfer_t arg = static_cast<xfer_t>(pid);
        Syscalls::get().vpectrl(sel(), KIF::Syscall::VCTRL_START, arg);

        write_state(pid, _next_sel, _eps, _resmng->sel(), _rbufcur, _rbufend, *_fds, *_ms);

        // notify child; it can start now
        write(fd[1], &byte, 1);
        close(fd[1]);
    }
    return Errors::NONE;

errorExec:
    close(tmp);
errorTemp:
    close(fd[0]);
    close(fd[1]);
    return Errors::OUT_OF_MEM;

}

}
