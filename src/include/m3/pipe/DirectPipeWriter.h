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

#include <m3/com/RecvGate.h>
#include <m3/com/SendGate.h>
#include <m3/vfs/File.h>
#include <m3/Exception.h>

#include <memory>

namespace m3 {

class DirectPipe;

/**
 * Writes into a previously constructed pipe.
 */
class DirectPipeWriter : public File {
    friend class DirectPipe;

public:
    struct State {
        explicit State(capsel_t caps, size_t size);

        ssize_t find_spot(size_t *len) noexcept;
        void read_replies();

        MemGate _mgate;
        RecvGate _rgate;
        SendGate _sgate;
        size_t _size;
        size_t _free;
        size_t _rdpos;
        size_t _wrpos;
        int _capacity;
        int _eof;
    };

    explicit DirectPipeWriter(capsel_t caps, size_t size, std::unique_ptr<State> &&state) noexcept;

public:
    virtual void stat(FileInfo &) const override {
        throw Exception(Errors::NOT_SUP);
    }
    virtual size_t seek(size_t, int) override {
        throw Exception(Errors::NOT_SUP);
    }

    virtual size_t read(void *, size_t) override {
        throw Exception(Errors::NOT_SUP);
    }
    virtual size_t write(const void *buffer, size_t count) override {
        return static_cast<size_t>(write(buffer, count, true));
    }

    // returns -1 when in non blocking mode and there is not enough space left in buffer
    ssize_t write(const void *buffer, size_t count, bool blocking);

    virtual Reference<File> clone() const override {
        return Reference<File>();
    }

    virtual char type() const noexcept override {
        return 'P';
    }
    virtual void delegate(VPE &vpe) override;
    virtual void serialize(Marshaller &m) override;
    static File *unserialize(Unmarshaller &um);

private:
    virtual void close() noexcept override;

    capsel_t _caps;
    size_t _size;
    std::unique_ptr<State> _state;
    bool _noeof;
};

}
