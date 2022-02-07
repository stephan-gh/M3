/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2021 Nils Asmussen, Barkhausen Institut
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
#include <assert.h>
#include <string.h>

namespace m3 {

class MsgBuf {
public:
    static constexpr size_t MAX_MSG_SIZE = 512;

    explicit MsgBuf() noexcept : _pos() {
    }

    MsgBuf(const MsgBuf &os) noexcept : _pos(os._pos) {
        if(_pos)
            memcpy(_bytes, os._bytes, _pos);
    }
    MsgBuf &operator=(const MsgBuf &os) noexcept {
        if(&os != this) {
            _pos = os._pos;
            if(_pos)
                memcpy(_bytes, os._bytes, _pos);
        }
        return *this;
    }

    void *bytes() noexcept {
        return _bytes;
    }
    const void *bytes() const noexcept {
        return _bytes;
    }
    size_t size() const noexcept {
        return _pos;
    }

    template<typename T>
    T &cast() noexcept {
        _pos = sizeof(T);
        return *reinterpret_cast<T*>(_bytes);
    }

    template<typename T>
    const T &get() const noexcept {
        assert(_pos >= sizeof(T));
        return *reinterpret_cast<const T*>(_bytes);
    }

    void set_size(size_t size) noexcept {
        _pos = size;
    }

private:
    uint8_t _bytes[MAX_MSG_SIZE];
    size_t _pos;
} PACKED ALIGNED(512);

}

#if defined(__host__)
#   include <base/arch/host/TCU.h>
#elif defined(__kachel__)
#   include <base/arch/kachel/TCU.h>
#else
#   error "Unsupported platform"
#endif
