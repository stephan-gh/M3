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
#include <m3/tiles/Activity.h>

namespace m3 {

/**
 * The base-class for a file that reads/writes from/to a pipe. Can't be instantiated.
 */
class SerialFile : public File {
public:
    explicit SerialFile() noexcept : File(FILE_RW) {
    }

    virtual Errors::Code try_stat(FileInfo &) const override {
        return Errors::NOT_SUP;
    }
    virtual size_t seek(size_t, int) override {
        throw Exception(Errors::NOT_SUP);
    }
    virtual void map(Reference<Pager> &, goff_t *, size_t, size_t, int, int) const override {
        throw Exception(Errors::NOT_SUP);
    }

    virtual size_t read(void *, size_t) override {
        // there is never anything to read
        return 0;
    }
    virtual size_t write(const void *buffer, size_t count) override {
        auto buf = reinterpret_cast<const char*>(buffer);
        while(count > 0) {
            ssize_t res = Machine::write(buf, count);
            if(res < 0)
                throw Exception(static_cast<Errors::Code>(-res));
            count -= static_cast<size_t>(res);
            buf += res;
        }
        return static_cast<size_t>(buf - reinterpret_cast<const char*>(buffer));
    }

    virtual Reference<File> clone() const override {
        return Reference<File>(new SerialFile());
    }

    virtual char type() const noexcept override {
        return 'S';
    }
    virtual void delegate(Activity &) override {
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
