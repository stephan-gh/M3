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

#include <base/Common.h>

#include <m3/com/GateStream.h>
#include <m3/vfs/File.h>
#include <m3/Exception.h>

#include <memory>

namespace m3 {

class DirectPipe;

/**
 * Reads from a previously constructed pipe.
 */
class DirectPipeReader : public File {
    friend class DirectPipe;

public:
    struct State {
        explicit State(capsel_t caps) noexcept;

        MemGate _mgate;
        RecvGate _rgate;
        size_t _pos;
        size_t _rem;
        size_t _pkglen;
        int _eof;
        std::unique_ptr<GateIStream> _is;
    };

    explicit DirectPipeReader(capsel_t caps, std::unique_ptr<State> &&state) noexcept;

public:
    virtual Errors::Code try_stat(FileInfo &) const override {
        return Errors::NOT_SUP;
    }
    virtual size_t seek(size_t, int) override {
        throw Exception(Errors::NOT_SUP);
    }
    virtual void map(Reference<Pager> &, goff_t *, size_t, size_t, int, int) const override {
        throw Exception(Errors::NOT_SUP);
    }

    virtual size_t read(void *buffer, size_t count) override {
        return static_cast<size_t>(read(buffer, count, true));
    }

    // returns -1 when in non blocking mode and there is no data to read
    ssize_t read(void *, size_t, bool blocking);

    virtual size_t write(const void *, size_t) override {
        throw Exception(Errors::NOT_SUP);
    }

    virtual Reference<File> clone() const override {
        return Reference<File>();
    }

    virtual char type() const noexcept override {
        return 'Q';
    }
    virtual void delegate(Activity &act) override;
    virtual void serialize(Marshaller &m) override;
    static File *unserialize(Unmarshaller &um);

private:
    virtual void close() noexcept override;

    bool _noeof;
    capsel_t _caps;
    std::unique_ptr<State> _state;
};

}
