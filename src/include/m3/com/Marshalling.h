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

#include <base/util/String.h>
#include <base/util/Math.h>
#include <base/TCU.h>

#include <m3/Exception.h>

#include <assert.h>

namespace m3 {

class Unmarshaller;

/**
 * The marshaller puts values into a buffer, which is for example used by GateOStream.
 */
class Marshaller {
public:
    explicit Marshaller(unsigned char *bytes, size_t total) noexcept
        : _bytecount(0),
          _bytes(bytes),
          _total(total) {
    }

    Marshaller(const Marshaller &) = default;
    Marshaller &operator=(const Marshaller &) = default;

    /**
     * @return the total number of bytes of the data
     */
    size_t total() const noexcept {
        return _bytecount;
    }
    /**
     * @return the bytes of the data
     */
    const unsigned char *bytes() const noexcept {
        return _bytes;
    }

    /**
     * Puts the given values into this Marshaller.
     *
     * @param val the first value
     * @param args the other values
     */
    template<typename T, typename... Args>
    void vput(const T &val, const Args &... args) noexcept {
        *this << val;
        vput(args...);
    }

    /**
     * Puts the given value into this Marshaller.
     *
     * @param value the value
     * @return *this
     */
    template<typename T>
    Marshaller & operator<<(const T& value) noexcept {
        assert(fits(_bytecount, sizeof(T)));
        *reinterpret_cast<xfer_t*>(_bytes + _bytecount) = (xfer_t)value;
        _bytecount += Math::round_up(sizeof(T), sizeof(xfer_t));
        return *this;
    }
    Marshaller & operator<<(const char *value) noexcept {
        return put_str(value, strlen(value) + 1);
    }
    Marshaller & operator<<(const StringRef& value) noexcept {
        return put_str(value.c_str(), value.length() + 1);
    }
    Marshaller & operator<<(const String& value) noexcept {
        return put_str(value.c_str(), value.length() + 1);
    }

    /**
     * Puts all remaining items (the ones that haven't been read yet) of <is> into this Marshaller.
     *
     * @param is the GateIStream
     * @return *this
     */
    void put(const Unmarshaller &is) noexcept;
    /**
     * Puts all items of <os> into this Marshaller.
     *
     * @param os the Marshaller
     * @return *this
     */
    void put(const Marshaller &os) noexcept;

protected:
    Marshaller & put_str(const char *value, size_t len) noexcept {
        assert(fits(_bytecount, len + sizeof(xfer_t)));
        unsigned char *start = const_cast<unsigned char*>(bytes());
        *reinterpret_cast<xfer_t*>(start + _bytecount) = len;
        memcpy(start + _bytecount + sizeof(xfer_t), value, len);
        _bytecount += Math::round_up(len + sizeof(xfer_t), sizeof(xfer_t));
        return *this;
    }

    // needed as recursion-end
    void vput() noexcept {
    }
    bool fits(size_t current, size_t bytes) noexcept {
        return current + bytes <= _total;
    }

    size_t _bytecount;
    unsigned char *_bytes;
    size_t _total;
};

/**
 * The unmarshaller reads values from a buffer, used e.g. in GateIStream.
 */
class Unmarshaller {
public:
    /**
     * Creates an object to read values from the given marshalled data.
     *
     * @param data the data to unmarshall
     * @param length the length of the data
     */
    explicit Unmarshaller(const unsigned char *data, size_t length) noexcept
        : _pos(0), _length(length), _data(data) {
    }

    Unmarshaller(const Unmarshaller &) = default;
    Unmarshaller &operator=(const Unmarshaller &) = default;

    /**
     * @return the current position, i.e. the offset of the unread data
     */
    size_t pos() const noexcept {
        return _pos;
    }
    /**
     * @return the length of the data in bytes
     */
    size_t length() const noexcept {
        return _length;
    }
    /**
     * @return the remaining bytes to read
     */
    size_t remaining() const noexcept {
        return length() - _pos;
    }
    /**
     * @return the data
     */
    const unsigned char *buffer() const noexcept {
        return _data;
    }

    void ignore(size_t bytes) noexcept {
        _pos += bytes;
    }

    /**
     * Pulls the given values out of this stream
     *
     * @param val the value to write to
     * @param args the other values to write to
     */
    template<typename T, typename... Args>
    void vpull(T &val, Args &... args) {
        *this >> val;
        vpull(args...);
    }

    /**
     * Pulls a value into <value>.
     *
     * @param value the value to write to
     * @return *this
     */
    template<typename T>
    Unmarshaller & operator>>(T &value) {
        if(_pos + sizeof(T) > length())
            throw Exception(Errors::INV_ARGS);
        value = (T)*reinterpret_cast<const xfer_t*>(_data + _pos);
        _pos += Math::round_up(sizeof(T), sizeof(xfer_t));
        return *this;
    }
    Unmarshaller & operator>>(String &value) {
        if(_pos + sizeof(xfer_t) > length())
            throw Exception(Errors::INV_ARGS);
        size_t len = *reinterpret_cast<const xfer_t*>(_data + _pos);
        _pos += sizeof(xfer_t);
        if(len == 0 || _pos + len > length())
            throw Exception(Errors::INV_ARGS);
        value.reset(reinterpret_cast<const char*>(_data + _pos), len - 1);
        _pos += Math::round_up(len, sizeof(xfer_t));
        return *this;
    }
    Unmarshaller & operator>>(StringRef &value) {
        if(_pos + sizeof(xfer_t) > length())
            throw Exception(Errors::INV_ARGS);
        size_t len = *reinterpret_cast<const xfer_t*>(_data + _pos);
        _pos += sizeof(xfer_t);
        if(len == 0 || _pos + len > length())
            throw Exception(Errors::INV_ARGS);
        value = StringRef(reinterpret_cast<const char*>(_data + _pos), len - 1);
        _pos += Math::round_up(len, sizeof(xfer_t));
        return *this;
    }

private:
    // needed as recursion-end
    void vpull() noexcept {
    }

    size_t _pos;
    size_t _length;
    const unsigned char *_data;
};

inline void Marshaller::put(const Unmarshaller &is) noexcept {
    assert(fits(_bytecount, is.remaining()));
    memcpy(const_cast<unsigned char*>(bytes()) + _bytecount, is.buffer() + is.pos(), is.remaining());
    _bytecount += is.remaining();
}
inline void Marshaller::put(const Marshaller &os) noexcept {
    assert(fits(_bytecount, os.total()));
    memcpy(const_cast<unsigned char*>(bytes()) + _bytecount, os.bytes(), os.total());
    _bytecount += os.total();
}

/**
 * The following templates are used to determine the size of given values in order to determine
 * the size of a message.
 */

template<typename T>
struct OStreamSize {
    static const size_t value = Math::round_up(sizeof(T), sizeof(xfer_t));
};
template<>
struct OStreamSize<StringRef> {
    static const size_t value = sizeof(xfer_t) + StringRef::DEFAULT_MAX_LEN;
};
template<>
struct OStreamSize<String> {
    static const size_t value = sizeof(xfer_t) + String::DEFAULT_MAX_LEN;
};
template<>
struct OStreamSize<const char*> {
    static const size_t value = sizeof(xfer_t) + StringRef::DEFAULT_MAX_LEN;
};
template<size_t N>
struct OStreamSize<char[N]> {
    static const size_t value = sizeof(xfer_t) + N;
};

template<typename T>
constexpr size_t _ostreamsize() {
    return OStreamSize<T>::value;
}
template<typename T1, typename T2, typename... Args>
constexpr size_t _ostreamsize() {
    return OStreamSize<T1>::value + _ostreamsize<T2, Args...>();
}

/**
 * @return the size required for <T1> and <Args>.
 */
template<typename T1, typename... Args>
constexpr size_t ostreamsize() {
    return _ostreamsize<T1, Args...>();
}

/**
 * @return the sum of the lengths <len> and <lens>, respecting alignment
 */
template<typename T>
constexpr size_t vostreamsize(T len) {
    return Math::round_up(len, sizeof(xfer_t));
}
template<typename T1, typename... Args>
constexpr size_t vostreamsize(T1 len, Args... lens) {
    return Math::round_up(len, sizeof(xfer_t)) + vostreamsize<Args...>(lens...);
}

static_assert(
    ostreamsize<int, float, int>() == sizeof(xfer_t) * 3,
    "failed");

static_assert(
    ostreamsize<short, StringRef>() == sizeof(xfer_t) + sizeof(xfer_t) + StringRef::DEFAULT_MAX_LEN,
    "failed");

static_assert(
    ostreamsize<short, String>() == sizeof(xfer_t) + sizeof(xfer_t) + StringRef::DEFAULT_MAX_LEN,
    "failed");

static_assert(
    ostreamsize<short, const char*>() == sizeof(xfer_t) + sizeof(xfer_t) + StringRef::DEFAULT_MAX_LEN,
    "failed");

static_assert(
    ostreamsize<short, char[5]>() == sizeof(xfer_t) + sizeof(xfer_t) + 5,
    "failed");

}
