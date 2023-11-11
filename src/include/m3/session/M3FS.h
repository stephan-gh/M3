/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

#include <m3/com/GateStream.h>
#include <m3/com/OpCodes.h>
#include <m3/com/RecvGate.h>
#include <m3/com/SendGate.h>
#include <m3/session/ClientSession.h>
#include <m3/vfs/FileSystem.h>
#include <m3/vfs/GenericFile.h>

#include <fs/internal.h>
#include <vector>

namespace m3 {

class GenericFile;

class M3FS : public ClientSession, public FileSystem {
    struct CachedEP {
        explicit CachedEP(size_t _id, EP *_ep) : id(_id), ep(_ep), file(-1) {
        }
        CachedEP(CachedEP &&c) : id(c.id), ep(c.ep), file(c.file) {
            c.ep = nullptr;
        }
        ~CachedEP();

        size_t id;
        EP *ep;
        ssize_t file;
    };

public:
    friend class GenericFile;

    explicit M3FS(size_t id, const std::string_view &service)
        : ClientSession(service, SelSpace::get().alloc_sels(2)),
          FileSystem(id),
          _gate(SendGate::bind(connect_for(Activity::own(), sel() + 1))),
          _eps() {
    }
    explicit M3FS(size_t id, capsel_t caps) noexcept
        : ClientSession(caps + 0),
          FileSystem(id),
          _gate(SendGate::bind(caps + 1)),
          _eps() {
    }

    const SendGate &gate() const noexcept {
        return _gate;
    }
    virtual char type() const noexcept override {
        return 'M';
    }

    virtual std::unique_ptr<GenericFile> open(const char *path, int perms) override;
    virtual void close(size_t file_id) override;
    virtual Errors::Code try_stat(const char *path, FileInfo &info) noexcept override;
    virtual Errors::Code try_mkdir(const char *path, mode_t mode) override;
    virtual Errors::Code try_rmdir(const char *path) override;
    virtual Errors::Code try_link(const char *oldpath, const char *newpath) override;
    virtual Errors::Code try_unlink(const char *path) override;
    virtual Errors::Code try_rename(const char *oldpath, const char *newpath) override;

    virtual void delegate(ChildActivity &act) override;
    virtual void serialize(Marshaller &m) override;
    static FileSystem *unserialize(Unmarshaller &um);

private:
    size_t get_ep();
    size_t delegate_ep(capsel_t sel);

    SendGate _gate;
    std::vector<CachedEP> _eps;
};

template<>
struct OStreamSize<FileInfo> {
    static const size_t value = 10 * sizeof(xfer_t);
};

static inline Unmarshaller &operator>>(Unmarshaller &u, FileInfo &info) noexcept {
    u >> info.devno >> info.inode >> info.mode >> info.links >> info.size >> info.lastaccess >>
        info.lastmod >> info.blocksize >> info.extents >> info.firstblock;
    return u;
}

static inline GateIStream &operator>>(GateIStream &is, FileInfo &info) noexcept {
    is >> info.devno >> info.inode >> info.mode >> info.links >> info.size >> info.lastaccess >>
        info.lastmod >> info.blocksize >> info.extents >> info.firstblock;
    return is;
}

static inline Marshaller &operator<<(Marshaller &m, const FileInfo &info) noexcept {
    m << info.devno << info.inode << info.mode << info.links << info.size << info.lastaccess
      << info.lastmod << info.blocksize << info.extents << info.firstblock;
    return m;
}

}
