/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019 Nils Asmussen, Barkhausen Institut
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

#include <m3/com/MemGate.h>
#include <m3/session/Pipes.h>
#include <m3/vfs/File.h>

namespace m3 {

class IndirectPipe {
public:
    explicit IndirectPipe(Pipes &pipes, MemCap &mem, size_t memsize, int flags = 0);
    ~IndirectPipe();

    /**
     * @return the file for the reader
     */
    GenericFile &reader() noexcept {
        return *_reader;
    }
    /**
     * Closes the read-end
     */
    void close_reader();

    /**
     * @return the file for the writer
     */
    GenericFile &writer() noexcept {
        return *_writer;
    }
    /**
     * Closes the write-end
     */
    void close_writer();

private:
    Pipes::Pipe _pipe;
    FileRef<GenericFile> _reader;
    FileRef<GenericFile> _writer;
};

}
