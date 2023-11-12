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

#include <m3/pipe/DirectPipe.h>
#include <m3/pipe/DirectPipeReader.h>
#include <m3/tiles/ChildActivity.h>

namespace m3 {

DirectPipeReader::State::State(capsel_t caps) noexcept
    : _mgate(MemGate::bind(caps + 1)),
      _rgate(RecvGate::bind(caps + 0)),
      _pos(),
      _rem(),
      _pkglen(static_cast<size_t>(-1)),
      _eof(0),
      _is() {
}

DirectPipeReader::DirectPipeReader(capsel_t caps, std::unique_ptr<State> &&state) noexcept
    : File(FILE_R),
      _noeof(),
      _caps(caps),
      _state(std::move(state)) {
}

void DirectPipeReader::remove() noexcept {
    if(_noeof)
        return;

    if(!_state)
        _state = std::make_unique<State>(_caps);
    if(~_state->_eof & DirectPipe::READ_EOF) {
        try {
            // if we have not fetched a message yet, do so now
            if(_state->_pkglen == static_cast<size_t>(-1)) {
                _state->_is = std::make_unique<GateIStream>(
                    receive_vmsg(_state->_rgate, _state->_pos, _state->_pkglen));
            }
            LOG(LogFlags::LibDirPipe, "[read] replying len={}"_cf, 0);
            reply_vmsg(*_state->_is, size_t(0));
        }
        catch(...) {
            // ignore
        }
        _state->_eof |= DirectPipe::READ_EOF;
    }
}

Option<size_t> DirectPipeReader::read(void *buffer, size_t count) {
    if(!_state)
        _state = std::make_unique<State>(_caps);
    if(_state->_eof)
        return Some(size_t(0));

    if(_state->_rem == 0) {
        if(_state->_pos > 0) {
            try {
                LOG(LogFlags::LibDirPipe, "[read] replying len={}"_cf, _state->_pkglen);
                reply_vmsg(*_state->_is, _state->_pkglen);
            }
            catch(...) {
                // maybe the writer stopped
            }
            _state->_is->finish();
            // Non blocking mode: Reset pos, so that reply is not sent a second time on next
            // invocation.
            _state->_pos = 0;
        }

        if(_blocking) {
            _state->_is = std::make_unique<GateIStream>(
                receive_vmsg(_state->_rgate, _state->_pos, _state->_pkglen));
        }
        else {
            const TCU::Message *msg = _state->_rgate.fetch();
            if(msg) {
                _state->_is = std::make_unique<GateIStream>(GateIStream(_state->_rgate, msg));
                _state->_is->vpull(_state->_pos, _state->_pkglen);
            }
            else
                return None;
        }
        _state->_rem = _state->_pkglen;
    }

    size_t amount = Math::min(count, _state->_rem);
    LOG(LogFlags::LibDirPipe, "[read] read from pos={}, len={}"_cf, _state->_pos, amount);
    if(amount == 0)
        _state->_eof |= DirectPipe::WRITE_EOF;
    else {
        // Skip data when no buffer is specified
        if(buffer)
            _state->_mgate.read(buffer, amount, _state->_pos);
        _state->_pos += amount;
        _state->_rem -= amount;
    }
    return Some(amount);
}

void DirectPipeReader::delegate(ChildActivity &act) {
    act.delegate(KIF::CapRngDesc(KIF::CapRngDesc::OBJ, _caps, 2));
}

void DirectPipeReader::serialize(Marshaller &m) {
    // we can't share the reader between two activities atm anyway, so don't serialize the current
    // state
    m << _caps;
}

File *DirectPipeReader::unserialize(Unmarshaller &um) {
    capsel_t caps;
    um >> caps;
    return new DirectPipeReader(caps, nullptr);
}

}
