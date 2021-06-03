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

#pragma once

#include <base/util/Reference.h>

#include <m3/session/ClientSession.h>
#include <m3/com/RecvGate.h>
#include <m3/com/SendGate.h>
#include <m3/com/GateStream.h>
#include <m3/vfs/FileSystem.h>
#include <m3/vfs/GenericFile.h>

#include <fs/internal.h>

namespace m3 {

class GenericFile;

class M3FS : public ClientSession, public FileSystem {
public:
    friend class GenericFile;

    enum Operation {
        FSTAT = GenericFile::STAT,
        SEEK = GenericFile::SEEK,
        NEXT_IN = GenericFile::NEXT_IN,
        NEXT_OUT = GenericFile::NEXT_OUT,
        COMMIT = GenericFile::COMMIT,
        SYNC = GenericFile::SYNC,
        CLOSE = GenericFile::CLOSE,
        STAT,
        MKDIR,
        RMDIR,
        LINK,
        UNLINK,
        RENAME,
        COUNT
    };

    explicit M3FS(const String &service)
        : ClientSession(service, VPE::self().alloc_sels(2)),
          FileSystem(),
          _gate(obtain_sgate()) {
    }
    explicit M3FS(capsel_t caps) noexcept
        : ClientSession(caps + 0),
          FileSystem(),
          _gate(SendGate::bind(caps + 1)) {
    }

    const SendGate &gate() const noexcept {
        return _gate;
    }
    virtual char type() const noexcept override {
        return 'M';
    }

    virtual Reference<File> open(const char *path, int perms) override;
    virtual Errors::Code try_stat(const char *path, FileInfo &info) noexcept override;
    virtual Errors::Code try_mkdir(const char *path, mode_t mode) override;
    virtual Errors::Code try_rmdir(const char *path) override;
    virtual Errors::Code try_link(const char *oldpath, const char *newpath) override;
    virtual Errors::Code try_unlink(const char *path) override;
    virtual Errors::Code try_rename(const char *oldpath, const char *newpath) override;

    virtual void delegate(VPE &vpe) override;
    virtual void serialize(Marshaller &m) override;
    static FileSystem *unserialize(Unmarshaller &um);

    // TODO wrong place. we should have a DataSpace session or something
    static size_t get_mem(ClientSession &sess, size_t *off, capsel_t *sel) {
        KIF::ExchangeArgs args;
        ExchangeOStream os(args);
        os << *off;
        args.bytes = os.total();

        KIF::CapRngDesc crd = sess.obtain(1, &args);

        size_t size;
        ExchangeIStream is(args);
        is >> *off >> size;
        *sel = crd.start();
        return size;
    }

private:
    SendGate obtain_sgate() {
        KIF::CapRngDesc crd(KIF::CapRngDesc::OBJ, sel() + 1);
        obtain_for(VPE::self(), crd);
        return SendGate::bind(crd.start());
    }

    SendGate _gate;
};

template<>
struct OStreamSize<FileInfo> {
    static const size_t value = 10 * sizeof(xfer_t);
};

static inline Unmarshaller &operator>>(Unmarshaller &u, FileInfo &info) noexcept {
    u >> info.devno >> info.inode >> info.mode >> info.links >> info.size >> info.lastaccess
      >> info.lastmod >> info.blocksize >> info.extents >> info.firstblock;
    return u;
}

static inline GateIStream &operator>>(GateIStream &is, FileInfo &info) noexcept {
    is >> info.devno >> info.inode >> info.mode >> info.links >> info.size >> info.lastaccess
      >> info.lastmod >> info.blocksize >> info.extents >> info.firstblock;
    return is;
}

static inline Marshaller &operator<<(Marshaller &m, const FileInfo &info) noexcept {
    m << info.devno << info.inode << info.mode << info.links << info.size << info.lastaccess
      << info.lastmod << info.blocksize << info.extents << info.firstblock;
    return m;
}

}
