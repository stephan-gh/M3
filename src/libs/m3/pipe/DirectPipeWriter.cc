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

#include <base/Log.h>

#include <m3/com/GateStream.h>
#include <m3/pipe/DirectPipe.h>
#include <m3/pipe/DirectPipeWriter.h>
#include <m3/tiles/ChildActivity.h>

namespace m3 {

DirectPipeWriter::State::State(capsel_t caps, size_t size)
    : _mgate(MemGate::bind(caps + 0)),
      _rgate(RecvGate::create(nextlog2<DirectPipe::MSG_BUF_SIZE>::val,
                              nextlog2<DirectPipe::MSG_SIZE>::val)),
      _sgate(SendCap::bind(caps + 1, &_rgate)),
      _size(size),
      _free(_size),
      _rdpos(),
      _wrpos(),
      _capacity(DirectPipe::MSG_BUF_SIZE / DirectPipe::MSG_SIZE),
      _eof() {
    _rgate.activate();
}

Option<size_t> DirectPipeWriter::State::find_spot(size_t *len) noexcept {
    if(_free == 0)
        return None;
    if(_wrpos >= _rdpos) {
        if(_wrpos < _size) {
            *len = Math::min(*len, _size - _wrpos);
            return Some(_wrpos);
        }
        if(_rdpos > 0) {
            *len = Math::min(*len, _rdpos);
            return Some(size_t(0));
        }
        return None;
    }
    if(_rdpos - _wrpos > 0) {
        *len = Math::min(*len, _rdpos - _wrpos);
        return Some(_wrpos);
    }
    return None;
}

void DirectPipeWriter::State::read_replies() {
    // read all expected responses
    if(~_eof & DirectPipe::READ_EOF) {
        size_t len = 1;
        int cap = DirectPipe::MSG_BUF_SIZE / DirectPipe::MSG_SIZE;
        while(len && _capacity < cap) {
            receive_vmsg(_rgate, len);
            LOG(LogFlags::LibDirPipe, "[shutdown] got len={}"_cf, len);
            _capacity++;
        }
    }
}

DirectPipeWriter::DirectPipeWriter(capsel_t caps, size_t size,
                                   std::unique_ptr<State> &&state) noexcept
    : File(FILE_W),
      _caps(caps),
      _size(size),
      _state(std::move(state)),
      _noeof() {
}

void DirectPipeWriter::remove() noexcept {
    if(_noeof)
        return;

    if(!_state)
        _state = std::make_unique<State>(_caps, _size);
    if(!_state->_eof) {
        try {
            write(nullptr, 0);
        }
        catch(...) {
            // ignore
        }
        _state->_eof |= DirectPipe::WRITE_EOF;
    }

    if(_state) {
        try {
            _state->read_replies();
        }
        catch(...) {
            // ignore
        }
    }
}

Option<size_t> DirectPipeWriter::write(const void *buffer, size_t count) {
    if(!_state)
        _state = std::make_unique<State>(_caps, _size);
    if(_state->_eof)
        return Some(size_t(0));

    size_t rem = count;
    const char *buf = reinterpret_cast<const char *>(buffer);
    do {
        size_t amount = rem;
        auto off = _state->find_spot(&amount);
        if(_state->_capacity == 0 || off.is_none()) {
            size_t len;
            if(_blocking) {
                receive_vmsg(_state->_rgate, len);
            }
            else {
                _state->_rgate.activate();
                const TCU::Message *msg = _state->_rgate.fetch();
                if(msg) {
                    GateIStream is(_state->_rgate, msg);
                    is.vpull(len);
                }
                else
                    return None;
            }
            LOG(LogFlags::LibDirPipe, "[write] got len={}"_cf, len);
            _state->_rdpos = (_state->_rdpos + len) % _state->_size;
            _state->_free += len;
            _state->_capacity++;
            if(len == 0) {
                _state->_eof |= DirectPipe::READ_EOF;
                return Some(size_t(0));
            }
            if(_state->_capacity == 0 || off.is_none()) {
                off = _state->find_spot(&amount);
                if(off.is_none())
                    return Some(size_t(0));
            }
        }

        size_t mem_off = off.unwrap();
        LOG(LogFlags::LibDirPipe, "[write] send pos={}, len={}"_cf, mem_off, amount);

        if(amount) {
            _state->_mgate.write(buf, amount, mem_off);
            _state->_wrpos = (mem_off + amount) % _size;
        }
        _state->_free -= amount;
        _state->_capacity--;
        try {
            send_vmsg(_state->_sgate.get(), mem_off, amount);
        }
        catch(...) {
            // maybe the reader stopped
            break;
        }
        rem -= amount;
        buf += amount;
    }
    while(rem > 0);
    return Some(static_cast<size_t>(buf - reinterpret_cast<const char *>(buffer)));
}

void DirectPipeWriter::delegate(ChildActivity &act) {
    act.delegate(KIF::CapRngDesc(KIF::CapRngDesc::OBJ, _caps, 2));
}

void DirectPipeWriter::serialize(Marshaller &m) {
    // we can't share the writer between two activities atm anyway, so don't serialize the current
    // state
    m << _caps << _size;
}

File *DirectPipeWriter::unserialize(Unmarshaller &um) {
    capsel_t caps;
    size_t size;
    um >> caps >> size;
    return new DirectPipeWriter(caps, size, std::unique_ptr<State>(new State(caps, size)));
}

}
