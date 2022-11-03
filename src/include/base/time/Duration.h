/*
 * Copyright (C) 2021 Nils Asmussen, Barkhausen Institut
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
#include <base/TCU.h>
#include <base/stream/Format.h>
#include <base/stream/OStream.h>

namespace m3 {

/**
 * A duration of time, represented in nanoseconds. Used in combination with TimeInstant.
 */
class TimeDuration {
public:
    using raw_t = uint64_t;

private:
    explicit TimeDuration(raw_t nanos) : _nanos(nanos) {
    }

public:
    /**
     * Creates a new and empty time duration (see TimeDuration::ZERO).
     */
    explicit TimeDuration() : _nanos() {
    }

    TimeDuration(const TimeDuration &c) = default;
    TimeDuration &operator=(const TimeDuration &c) = default;

    /**
     * A time duration that lasts for one nanosecond.
     */
    static const TimeDuration NANOSECOND;
    /**
     * A time duration that lasts for one microsecond.
     */
    static const TimeDuration MICROSECOND;
    /**
     * A time duration that lasts for one millisecond.
     */
    static const TimeDuration MILLISECOND;
    /**
     * A time duration that lasts for one second.
     */
    static const TimeDuration SECOND;
    /**
     * The maximum representable time duration.
     */
    static const TimeDuration MAX;
    /**
     * An empty time duration.
     */
    static const TimeDuration ZERO;

    /**
     * @param raw the raw time duration (in nanoseconds)
     * @return a new TimeDuration from given raw value
     */
    static TimeDuration from_raw(raw_t raw) {
        return TimeDuration(raw);
    }
    /**
     * @param nanos the time duration
     * @return a new TimeDuration with given number of nanoseconds
     */
    static TimeDuration from_nanos(raw_t nanos) {
        return TimeDuration(nanos);
    }
    /**
     * @param micros the time duration
     * @return a new TimeDuration with given number of microseconds
     */
    static TimeDuration from_micros(raw_t micros) {
        return TimeDuration(micros * MICROSECOND._nanos);
    }
    /**
     * @param millis the time duration
     * @return a new TimeDuration with given number of milliseconds
     */
    static TimeDuration from_millis(raw_t millis) {
        return TimeDuration(millis * MILLISECOND._nanos);
    }
    /**
     * @param secs the time duration
     * @return a new TimeDuration with given number of seconds
     */
    static TimeDuration from_secs(raw_t secs) {
        return TimeDuration(secs * SECOND._nanos);
    }

    /**
     * @return the underlying raw value (nanoseconds)
     */
    raw_t as_raw() const {
        return _nanos;
    }
    /**
     * @return the time duration as nanoseconds
     */
    raw_t as_nanos() const {
        return _nanos;
    }
    /**
     * @return the time duration as microseconds
     */
    raw_t as_micros() const {
        return _nanos / MICROSECOND._nanos;
    }
    /**
     * @return the time duration as milliseconds
     */
    raw_t as_millis() const {
        return _nanos / MILLISECOND._nanos;
    }
    /**
     * @return the time duration as seconds
     */
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

    void format(OStream &os, const FormatSpecs &) const {
        if(as_raw() >= TimeDuration::SECOND.as_raw())
            format_to(os, "{} ms"_cf, as_millis());
        else if(as_raw() >= TimeDuration::MILLISECOND.as_raw())
            format_to(os, "{} us"_cf, as_micros());
        else
            format_to(os, "{} ns"_cf, as_nanos());
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
static inline bool operator<(const TimeDuration &lhs, const TimeDuration &rhs) {
    return lhs.as_raw() < rhs.as_raw();
}
static inline bool operator>(const TimeDuration &lhs, const TimeDuration &rhs) {
    return operator<(rhs, lhs);
}
static inline bool operator<=(const TimeDuration &lhs, const TimeDuration &rhs) {
    return !operator>(lhs, rhs);
}
static inline bool operator>=(const TimeDuration &lhs, const TimeDuration &rhs) {
    return !operator<(lhs, rhs);
}

/**
 * A duration in cycles. Used in combination with CycleInstant.
 */
class CycleDuration {
public:
    using raw_t = uint64_t;

private:
    explicit CycleDuration(raw_t cycles) : _cycles(cycles) {
    }

public:
    /**
     * Creates a new cycle duration with 0 cycles
     */
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

    /**
     * @return the underlying raw value (in cycles)
     */
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

    template<typename O>
    void format(O &out, const FormatSpecs &) const {
        format_to(out, "{} cycles"_cf, as_raw());
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
static inline bool operator<(const CycleDuration &lhs, const CycleDuration &rhs) {
    return lhs.as_raw() < rhs.as_raw();
}
static inline bool operator>(const CycleDuration &lhs, const CycleDuration &rhs) {
    return operator<(rhs, lhs);
}
static inline bool operator<=(const CycleDuration &lhs, const CycleDuration &rhs) {
    return !operator>(lhs, rhs);
}
static inline bool operator>=(const CycleDuration &lhs, const CycleDuration &rhs) {
    return !operator<(lhs, rhs);
}

}
