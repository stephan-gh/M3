/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2020 Nils Asmussen, Barkhausen Institut
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

#include <base/stream/OStream.h>

#include <stdlib.h>

namespace m3 {

/**
 * Output-stream that writes to a string
 */
class OStringStream : public OStream {
    static const size_t DEFAULT_SIZE = 64;

public:
    /**
     * Constructor that allocates and automatically increases the string while
     * writing to the stream
     */
    explicit OStringStream()
        : OStream(),
          _dynamic(true),
          _dst(static_cast<char *>(malloc(DEFAULT_SIZE))),
          _max(_dst ? DEFAULT_SIZE : 0),
          _pos() {
        if(_dst)
            *_dst = '\0';
    }

    /**
     * Constructor that writes into the given string
     *
     * @param dst the string
     * @param max the size of <dst>
     */
    explicit OStringStream(char *dst, size_t max)
        : OStream(),
          _dynamic(false),
          _dst(dst),
          _max(max),
          _pos() {
        *_dst = '\0';
    }

    /**
     * Destroys the string, if it has been allocated here
     */
    virtual ~OStringStream() {
        if(_dynamic)
            free(_dst);
    }

    /**
     * Resets the internal position
     */
    void reset() {
        _pos = 0;
    }

    /**
     * @return the length of the string
     */
    size_t length() const {
        return _pos;
    }
    /**
     * @return the string
     */
    const char *str() const {
        return _dst ? _dst : "";
    }

    virtual void write(char c) override {
        // increase the buffer, if necessary
        if(_pos + 1 >= _max && _dynamic) {
            _max *= 2;
            _dst = static_cast<char *>(realloc(_dst, _max));
            // ensure that we do no longer write into the string
            if(!_dst)
                _max = 0;
        }
        // write into the buffer if there is still enough room
        if(_pos + 1 < _max) {
            _dst[_pos++] = c;
            _dst[_pos] = '\0';
        }
    }

private:
    bool _dynamic;
    char *_dst;
    size_t _max;
    size_t _pos;
};

}
