/*
 * Copyright (C) 2015, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/Common.h>
#include <base/stream/OStream.h>
#include <base/util/Math.h>
#include <base/TCU.h>

#include <algorithm>

class VarRingBuf {
public:
    explicit VarRingBuf(size_t size)
        : _size(size),
          _rdpos(),
          _wrpos(),
          _last(size) {
    }

    bool empty() const {
        return _rdpos == _wrpos;
    }
    size_t size() const {
        return _size;
    }

    /**
     * Determines the current write position.
     *
     * @param size the amount of bytes to write
     * @return the write position of the buffer, or -1 if the buffer does not has <size> bytes of consecutive free memory
     */
    ssize_t get_write_pos(size_t size) {
        if(_wrpos >= _rdpos) {
            if(_size - _wrpos >= size)
                return static_cast<ssize_t>(_wrpos);
            else if(_rdpos > size)
                return 0;
        }
        else if(_rdpos - _wrpos > size)
            return static_cast<ssize_t>(_wrpos);
        return -1;
    }

    /**
     * Determines the read position and the amount of bytes available to read.
     *
     * @param size the amount of bytes to read (*<size> = min(*<size>, available bytes))
     * @return the read position of the buffer, or -1 if the buffer is empty
     */
    ssize_t get_read_pos(size_t *size) {
        if(_wrpos == _rdpos)
            return -1;

        size_t rpos = _rdpos;
        if(rpos == _last)
            rpos = 0;
        if(_wrpos > rpos)
            *size = std::min(_wrpos - rpos,*size);
        else
            *size = std::min(std::min(_size, _last) - rpos,*size);
        return static_cast<ssize_t>(rpos);
    }

    /**
     * Advances the write position by <size>.
     *
     * @param req_size the number of bytes passed to get_write_pos
     *    Used to detect a potential wrap around to zero done by get_write_pos,
     *    even if <size> would not require one.
     * @param size the number of bytes to advance the write position by
     */
    void push(size_t req_size, size_t size) {
        if(_wrpos >= _rdpos) {
            if(_size - _wrpos >= req_size)
                _wrpos += size;
            else if(_rdpos > req_size && size > 0) {
                _last = _wrpos;
                _wrpos = size;
            }
        }
        else if(_rdpos - _wrpos > req_size)
            _wrpos += size;
    }

    /**
     * Advances the read position by <size>.
     *
     * @param size the number of bytes to advance the read position by
     */
    void pull(size_t size) {
        assert(!empty());
        if(_rdpos == _last) {
            _rdpos = 0;
            _last = _size;
        }
        _rdpos += size;
    }

    friend m3::OStream &operator<<(m3::OStream &os, const VarRingBuf &r) {
        os << "RingBuf[rd=" << r._rdpos << ",wr=" << r._wrpos << ",last=" << r._last << "]";
        return os;
    }

private:
    size_t _size;
    size_t _rdpos;
    size_t _wrpos;
    size_t _last;
};
