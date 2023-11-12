/*
 * Copyright (C) 2016-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <m3/pipe/IndirectPipe.h>
#include <m3/tiles/Activity.h>
#include <m3/vfs/FileTable.h>

namespace m3 {

IndirectPipe::IndirectPipe(Pipes &pipes, MemCap &mem, size_t memsize, int flags)
    : _pipe(pipes.create_pipe(mem, memsize)),
      _reader(_pipe.create_channel(true, flags)),
      _writer(_pipe.create_channel(false, flags)) {
}

IndirectPipe::~IndirectPipe() {
    try {
        close_reader();
    }
    catch(...) {
        // ignore
    }

    try {
        close_writer();
    }
    catch(...) {
        // ignore
    }
}

void IndirectPipe::close_reader() {
    _reader.reset();
}

void IndirectPipe::close_writer() {
    _writer.reset();
}

}
