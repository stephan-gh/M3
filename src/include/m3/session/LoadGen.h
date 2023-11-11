/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/CPU.h>
#include <base/Errors.h>
#include <base/KIF.h>

#include <m3/com/GateStream.h>
#include <m3/com/MemGate.h>
#include <m3/com/OpCodes.h>
#include <m3/com/RecvGate.h>
#include <m3/com/SendGate.h>
#include <m3/session/ClientSession.h>
#include <m3/tiles/OwnActivity.h>

#include <memory>

namespace m3 {

class LoadGen : public ClientSession {
public:
    class Channel {
    public:
        explicit Channel(capsel_t sels, size_t memsize)
            : _off(),
              _rem(),
              _rgate(RecvGate::create(nextlog2<64>::val, nextlog2<64>::val)),
              _scap(SendCap::create(&_rgate, SendGateArgs().credits(1).sel(sels + 0))),
              _mgate(MemGate::create_global(memsize, MemGate::RW, sels + 1)),
              _is() {
            _rgate.activate();
        }

        void wait() {
            _is = std::make_unique<GateIStream>(receive_msg(_rgate));
            *_is >> _rem;
            _off = 0;
        }

        size_t pull(void *, size_t size) noexcept {
            size_t amount = Math::min(size, _rem);
            if(amount == 0) {
                _off = 0;
                return 0;
            }
            if(size > 2)
                CPU::compute(size / 2);
            // _mgate.read(buf, amount, _off);
            _off += amount;
            _rem -= amount;
            return amount;
        }

        void push(const void *, size_t size) noexcept {
            // TODO allow larger replies than our mgate
            if(size > 4)
                CPU::compute(size / 4);
            // _mgate.write(buf, size, _off);
            _off += size;
        }

        void reply() {
            reply_vmsg(*_is, opcodes::LoadGen::RESPONSE, _off);
        }

    private:
        size_t _off;
        size_t _rem;
        RecvGate _rgate;
        SendCap _scap;
        MemGate _mgate;
        std::unique_ptr<GateIStream> _is;
    };

    explicit LoadGen(const std::string_view &name)
        : ClientSession(name),
          _sgate(SendGate::bind(obtain(1).start())) {
    }

    void start(uint count) {
        send_receive_vmsg(_sgate, opcodes::LoadGen::START, count);
    }

    Channel *create_channel(size_t memsize) {
        capsel_t sels = SelSpace::get().alloc_sels(2);
        auto chan = new Channel(sels, memsize);
        delegate(KIF::CapRngDesc(KIF::CapRngDesc::OBJ, sels, 2));
        return chan;
    }

private:
    SendGate _sgate;
};

}
