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
#include <m3/pes/VPE.h>

#include <sys/fcntl.h>
#include <sys/stat.h>
#include <stdlib.h>
#include <unistd.h>

namespace m3 {

class Chan {
public:
    explicit Chan() : fds() {
        if(::pipe(fds) == -1)
            throw Exception(Errors::OUT_OF_MEM);
    }
    ~Chan() {
        if(fds[0] != -1)
            close(fds[0]);
        if(fds[1] != -1)
            close(fds[1]);
    }

    void wait() {
        close(fds[1]);
        fds[1] = -1;

        // wait until parent notifies us
        uint8_t dummy;
        read(fds[0], &dummy, sizeof(dummy));
        close(fds[0]);
        fds[0] = -1;
    }

    void signal() {
        close(fds[0]);
        fds[0] = -1;

        // notify child; it can start now
        uint8_t dummy = 0;
        write(fds[1], &dummy, sizeof(dummy));
        close(fds[1]);
        fds[1] = -1;
    }

    int fds[2];
};

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

static void write_state(pid_t pid, capsel_t nextsel, capsel_t rmng,
                        uint64_t rbufcur, uint64_t rbufend,
                        FileTable &files, MountTable &mounts) {
    size_t len = STATE_BUF_SIZE;
    std::unique_ptr<unsigned char[]> buf(new unsigned char[len]);

    write_file(pid, "nextsel", nextsel);
    write_file(pid, "rmng", rmng);

    Marshaller m(buf.get(), len);
    m << rbufcur << rbufend;
    write_file(pid, "rbufs", buf.get(), m.total());

    len = mounts.serialize(buf.get(), STATE_BUF_SIZE);
    write_file(pid, "ms", buf.get(), len);

    len = files.serialize(buf.get(), STATE_BUF_SIZE);
    write_file(pid, "fds", buf.get(), len);
}

void VPE::init_state() {
    if(env()->first_sel() != 0)
        _next_sel = env()->first_sel();
    else
        read_from("nextsel", &_next_sel);

    capsel_t rmng_sel;
    if(read_from("rmng", &rmng_sel))
        _resmng.reset(new ResMng(rmng_sel));
    else if(_resmng == nullptr)
        _resmng.reset(new ResMng(ObjCap::INVALID));

    size_t len = sizeof(uint64_t) * 2;
    uint8_t buf[len];
    if(read_from("rbufs", buf, len)) {
        Unmarshaller um(buf, len);
        um >> _rbufcur >> _rbufend;
    }

    _epmng.reset();
}

void VPE::init_fs() {
    // don't free them; we don't want to revoke caps
    _fds = nullptr;
    _ms = nullptr;

    size_t len = STATE_BUF_SIZE;
    std::unique_ptr<char[]> buf(new char[len]);

    memset(buf.get(), 0, len);
    if(read_from("ms", buf.get(), len))
        _ms.reset(MountTable::unserialize(buf.get(), len));

    len = STATE_BUF_SIZE;
    memset(buf.get(), 0, len);
    if(read_from("fds", buf.get(), len))
        _fds.reset(FileTable::unserialize(buf.get(), len));

    // DTU is ready now; notify parent
    int pipefd;
    if(read_from("dturdy", &pipefd)) {
        uint8_t dummy = 0;
        write(pipefd, &dummy, sizeof(dummy));
        close(pipefd);
    }
}

void VPE::run(void *lambda) {
    Chan p2c, c2p;

    int pid = fork();
    if(pid == -1)
        throw Exception(Errors::OUT_OF_MEM);
    else if(pid == 0) {
        p2c.wait();

        env()->reset();
        VPE::self().init_state();
        VPE::self().init_fs();

        c2p.signal();

        std::function<int()> *func = reinterpret_cast<std::function<int()>*>(lambda);
        (*func)();
        exit(0);
    }
    else {
        // let the kernel create the config-file etc. for the given pid
        xfer_t arg = static_cast<xfer_t>(pid);
        Syscalls::vpe_ctrl(sel(), KIF::Syscall::VCTRL_START, arg);

        write_state(pid, _next_sel, _resmng->sel(), _rbufcur, _rbufend, *_fds, *_ms);

        p2c.signal();
        // wait until the DTU sockets have been binded
        c2p.wait();
    }
}

void VPE::exec(int argc, const char **argv) {
    static char buffer[8192];
    char templ[] = "/tmp/m3-XXXXXX";
    int tmp, pid;
    size_t res;
    Chan p2c, c2p;

    FileRef bin(argv[0], FILE_R);
    tmp = mkstemp(templ);
    if(tmp < 0)
        throw Exception(Errors::OUT_OF_MEM);

    // copy executable from M3-fs to a temp file
    while((res = bin->read(buffer, sizeof(buffer))) > 0)
        write(tmp, buffer, static_cast<size_t>(res));

    pid = fork();
    if(pid == -1) {
        close(tmp);
        throw Exception(Errors::OUT_OF_MEM);
    }
    else if(pid == 0) {
        // wait until the env file has been written by the kernel
        p2c.wait();

        // tell child about fd to notify parent if DTU is ready
        write_file(getpid(), "dturdy", static_cast<uint64_t>(c2p.fds[1]));
        close(c2p.fds[0]);

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
        close(tmp);

        // let the kernel create the config-file etc. for the given pid
        xfer_t arg = static_cast<xfer_t>(pid);
        Syscalls::vpe_ctrl(sel(), KIF::Syscall::VCTRL_START, arg);

        write_state(pid, _next_sel, _resmng->sel(), _rbufcur, _rbufend, *_fds, *_ms);

        p2c.signal();
        // wait until the DTU sockets have been binded
        c2p.wait();
    }
}

}
