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

#include <m3/pipe/DirectPipe.h>
#include <m3/pipe/DirectPipeReader.h>
#include <m3/pipe/DirectPipeWriter.h>
#include <m3/vfs/FileTable.h>

namespace m3 {

DirectPipe::DirectPipe(Activity &rd, Activity &wr, MemGate &mem, size_t size)
    : _rd(rd),
      _wr(wr),
      _size(size),
      _rgate(RecvGate::create(Activity::self().alloc_sels(4), nextlog2<MSG_BUF_SIZE>::val, nextlog2<MSG_SIZE>::val)),
      _rmem(mem.derive_for(Activity::self().sel(), _rgate.sel() + 1, 0, size, MemGate::R)),
      _wmem(mem.derive_for(Activity::self().sel(), _rgate.sel() + 2, 0, size, MemGate::W)),
      _sgate(SendGate::create(&_rgate, SendGateArgs().credits(CREDITS).sel(_rgate.sel() + 3))),
      _rdfd(),
      _wrfd() {
    std::unique_ptr<DirectPipeReader::State> rstate(
        &rd == &Activity::self() ? new DirectPipeReader::State(caps()) : nullptr);
    _rdfd = Activity::self().files()->alloc(Reference<File>(
        new DirectPipeReader(caps(), std::move(rstate))));

    std::unique_ptr<DirectPipeWriter::State> wstate(
        &wr == &Activity::self() ? new DirectPipeWriter::State(caps() + 2, _size) : nullptr);
    _wrfd = Activity::self().files()->alloc(Reference<File>(
        new DirectPipeWriter(caps() + 2, _size, std::move(wstate))));
}

DirectPipe::~DirectPipe() {
    try {
        close_writer();
    }
    catch(...) {
        // ignore
    }

    try {
        close_reader();
    }
    catch(...) {
        // ignore
    }
}

void DirectPipe::close_reader() {
    Reference<File> frd = Activity::self().files()->get(_rdfd);
    DirectPipeReader *rd = static_cast<DirectPipeReader*>(frd.get());
    if(rd) {
        // don't send EOF, if we are not reading
        if(&_rd != &Activity::self())
            rd->_noeof = true;
    }
    Activity::self().files()->remove(_rdfd);
}

void DirectPipe::close_writer() {
    Reference<File> fwr = Activity::self().files()->get(_wrfd);
    DirectPipeWriter *wr = static_cast<DirectPipeWriter*>(fwr.get());
    if(wr) {
        // don't send EOF, if we are not writing
        if(&_wr != &Activity::self())
            wr->_noeof = true;
    }
    Activity::self().files()->remove(_wrfd);
}

}
