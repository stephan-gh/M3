/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#pragma once

#include <base/stream/OStream.h>
#include <base/CPU.h>
#include <base/TCU.h>

namespace m3 {

class TimeDuration {
public:
    using raw_t = uint64_t;

private:
    explicit TimeDuration(raw_t nanos) : _nanos(nanos) {
    }

public:
    explicit TimeDuration() : _nanos() {
    }

    TimeDuration(const TimeDuration &c) = default;
    TimeDuration &operator=(const TimeDuration &c) = default;

    static const TimeDuration NANOSECOND;
    static const TimeDuration MICROSECOND;
    static const TimeDuration MILLISECOND;
    static const TimeDuration SECOND;
    static const TimeDuration MAX;
    static const TimeDuration ZERO;

    static TimeDuration from_raw(raw_t raw) {
        return TimeDuration(raw);
    }
    static TimeDuration from_nanos(raw_t nanos) {
        return TimeDuration(nanos);
    }
    static TimeDuration from_micros(raw_t micros) {
        return TimeDuration(micros * MICROSECOND._nanos);
    }
    static TimeDuration from_millis(raw_t millis) {
        return TimeDuration(millis * MILLISECOND._nanos);
    }
    static TimeDuration from_secs(raw_t secs) {
        return TimeDuration(secs * SECOND._nanos);
    }

    raw_t as_raw() const {
        return _nanos;
    }
    raw_t as_nanos() const {
        return _nanos;
    }
    raw_t as_micros() const {
        return _nanos / MICROSECOND._nanos;
    }
    raw_t as_millis() const {
        return _nanos / MILLISECOND._nanos;
    }
    raw_t as_secs() const {
        return _nanos / SECOND._nanos;
    }

    friend TimeDuration operator+(TimeDuration a, const TimeDuration &b) {
        a += b;
        return a;
    }
    TimeDuration &operator+=(const TimeDuration &t) {
        _nanos += t._nanos;
        return *this;
    }

    friend TimeDuration operator-(TimeDuration a, const TimeDuration &b) {
        a -= b;
        return a;
    }
    TimeDuration &operator-=(const TimeDuration &t) {
        _nanos -= t._nanos;
        return *this;
    }

    template<typename T>
    friend TimeDuration operator*(TimeDuration a, T factor) {
        a *= factor;
        return a;
    }
    template<typename T>
    TimeDuration &operator*=(T factor) {
        _nanos *= factor;
        return *this;
    }

    template<typename T>
    friend TimeDuration operator/(TimeDuration a, T divisor) {
        a /= divisor;
        return a;
    }
    template<typename T>
    TimeDuration &operator/=(T divisor) {
        _nanos /= divisor;
        return *this;
    }

    friend OStream &operator<<(OStream &os, const TimeDuration &t) {
        if(t._nanos >= SECOND._nanos)
            os << t.as_millis() << " ms";
        else if(t._nanos >= MILLISECOND._nanos)
            os << t.as_micros() << " us";
        else
            os << t.as_nanos() << " ns";
        return os;
    }

private:
    raw_t _nanos;
};

static inline bool operator==(const TimeDuration &lhs, const TimeDuration &rhs) {
    return lhs.as_raw() == rhs.as_raw();
}
static inline bool operator!=(const TimeDuration &lhs, const TimeDuration &rhs) {
    return !operator==(lhs, rhs);
}
static inline bool operator< (const TimeDuration &lhs, const TimeDuration &rhs) {
    return lhs.as_raw() < rhs.as_raw();
}
static inline bool operator> (const TimeDuration &lhs, const TimeDuration &rhs) {
    return  operator< (rhs, lhs);
}
static inline bool operator<=(const TimeDuration &lhs, const TimeDuration &rhs) {
    return !operator> (lhs, rhs);
}
static inline bool operator>=(const TimeDuration &lhs, const TimeDuration &rhs) {
    return !operator< (lhs, rhs);
}

class CycleDuration {
public:
    using raw_t = uint64_t;

private:
    explicit CycleDuration(raw_t cycles) : _cycles(cycles) {
    }

public:
    explicit CycleDuration() : _cycles() {
    }

    CycleDuration(const CycleDuration &c) = default;
    CycleDuration &operator=(const CycleDuration &c) = default;

    /**
     * @return a new duration from given cycle count.
     */
    static CycleDuration from_raw(raw_t cycles) {
        return CycleDuration(cycles);
    }

    raw_t as_raw() const {
        return _cycles;
    }

    friend CycleDuration operator+(CycleDuration a, const CycleDuration &b) {
        a += b;
        return a;
    }
    CycleDuration &operator+=(const CycleDuration &t) {
        _cycles += t._cycles;
        return *this;
    }

    friend CycleDuration operator-(CycleDuration a, const CycleDuration &b) {
        a -= b;
        return a;
    }
    CycleDuration &operator-=(const CycleDuration &t) {
        _cycles -= t._cycles;
        return *this;
    }

    template<typename T>
    friend CycleDuration operator*(CycleDuration a, T factor) {
        a *= factor;
        return a;
    }
    template<typename T>
    CycleDuration &operator*=(T factor) {
        _cycles *= factor;
        return *this;
    }

    template<typename T>
    friend CycleDuration operator/(CycleDuration a, T divisor) {
        a /= divisor;
        return a;
    }
    template<typename T>
    CycleDuration &operator/=(T divisor) {
        _cycles /= divisor;
        return *this;
    }

    friend OStream &operator<<(OStream &os, const CycleDuration &t) {
        os << t._cycles << " cycles";
        return os;
    }

private:
    raw_t _cycles;
};

static inline bool operator==(const CycleDuration &lhs, const CycleDuration &rhs) {
    return lhs.as_raw() == rhs.as_raw();
}
static inline bool operator!=(const CycleDuration &lhs, const CycleDuration &rhs) {
    return !operator==(lhs, rhs);
}
static inline bool operator< (const CycleDuration &lhs, const CycleDuration &rhs) {
    return lhs.as_raw() < rhs.as_raw();
}
static inline bool operator> (const CycleDuration &lhs, const CycleDuration &rhs) {
    return  operator< (rhs, lhs);
}
static inline bool operator<=(const CycleDuration &lhs, const CycleDuration &rhs) {
    return !operator> (lhs, rhs);
}
static inline bool operator>=(const CycleDuration &lhs, const CycleDuration &rhs) {
    return !operator< (lhs, rhs);
}

}
