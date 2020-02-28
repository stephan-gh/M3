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

#include <base/Machine.h>

#include <m3/vfs/File.h>
#include <m3/Exception.h>
#include <m3/pes/VPE.h>

namespace m3 {

/**
 * The base-class for a file that reads/writes from/to a pipe. Can't be instantiated.
 */
class SerialFile : public File {
public:
    explicit SerialFile() noexcept : File(FILE_RW) {
    }

    virtual void stat(FileInfo &) const override {
        throw Exception(Errors::NOT_SUP);
    }
    virtual size_t seek(size_t, int) override {
        throw Exception(Errors::NOT_SUP);
    }

    virtual size_t read(void *buffer, size_t count) override {
        ssize_t res = Machine::read(reinterpret_cast<char*>(buffer), count);
        if(res < 0)
            throw Exception(static_cast<Errors::Code>(-res));
        return static_cast<size_t>(res);
    }
    virtual size_t write(const void *buffer, size_t count) override {
        auto buf = reinterpret_cast<const char*>(buffer);
        while(count > 0) {
            size_t amount = Math::min(Machine::BUF_SIZE, count);
            int res = Machine::write(buf, amount);
            if(res < 0)
                throw Exception(static_cast<Errors::Code>(-res));
            count -= amount;
            buf += amount;
        }
        return static_cast<size_t>(buf - reinterpret_cast<const char*>(buffer));
    }

    virtual Reference<File> clone() const override {
        return Reference<File>(new SerialFile());
    }

    virtual char type() const noexcept override {
        return 'S';
    }
    virtual void delegate(VPE &) override {
        // nothing to do
    }
    virtual void serialize(Marshaller &) override {
        // nothing to do
    }
    static SerialFile *unserialize(Unmarshaller &) {
        return new SerialFile();
    }

    virtual void close() noexcept override {
    }
};

}
