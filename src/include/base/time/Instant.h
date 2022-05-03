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

#include <base/time/Duration.h>

namespace m3 {

/**
 * A measurement of time, represented in nanoseconds. Useful in combination with TimeDuration.
 */
class TimeInstant {
private:
    explicit TimeInstant(uint64_t nanos) : _nanos(nanos) {
    }

public:
    using Duration = TimeDuration;

    TimeInstant(const TimeInstant &c) = default;
    TimeInstant &operator=(const TimeInstant &c) = default;

    /**
     * @return an instant corresponding to "now".
     */
    static TimeInstant now() {
        return TimeInstant::from_nanos(TCU::get().nanotime());
    }
    /**
     * @return a new time instant from the given number of nanoseconds.
     */
    static TimeInstant from_nanos(uint64_t nanos) {
        return TimeInstant(nanos);
    }

    /**
     * @return the time instant in nanoseconds
     */
    uint64_t as_nanos() const {
        return _nanos;
    }

    /**
     * @return the amount of time elapsed from another instant to this one.
     */
    TimeDuration duration_since(const TimeInstant &earlier) const {
        assert(_nanos >= earlier._nanos);
        return TimeDuration::from_nanos(_nanos - earlier._nanos);
    }

    /**
     * @return the amount of time elapsed since this instant was created.
     */
    TimeDuration elapsed() const {
        return TimeDuration::from_nanos(now()._nanos - _nanos);
    }

    friend TimeInstant operator+(TimeInstant i, const TimeDuration &d) {
        i._nanos += d.as_nanos();
        return i;
    }

private:
    uint64_t _nanos;
};

static inline bool operator==(const TimeInstant &lhs, const TimeInstant &rhs) {
    return lhs.as_nanos() == rhs.as_nanos();
}
static inline bool operator!=(const TimeInstant &lhs, const TimeInstant &rhs) {
    return !operator==(lhs, rhs);
}
static inline bool operator<(const TimeInstant &lhs, const TimeInstant &rhs) {
    return lhs.as_nanos() < rhs.as_nanos();
}
static inline bool operator>(const TimeInstant &lhs, const TimeInstant &rhs) {
    return operator<(rhs, lhs);
}
static inline bool operator<=(const TimeInstant &lhs, const TimeInstant &rhs) {
    return !operator>(lhs, rhs);
}
static inline bool operator>=(const TimeInstant &lhs, const TimeInstant &rhs) {
    return !operator<(lhs, rhs);
}

/**
 * A measurement of cycles. Useful in combination with CycleDuration.
 */
class CycleInstant {
private:
    explicit CycleInstant(uint64_t cycles) : _cycles(cycles) {
    }

public:
    using Duration = CycleDuration;

    CycleInstant(const CycleInstant &c) = default;
    CycleInstant &operator=(const CycleInstant &c) = default;

    /**
     * @return an instant corresponding to "now".
     */
    static CycleInstant now() {
        return CycleInstant::from_cycles(CPU::elapsed_cycles());
    }
    /**
     * @return an instant from the given number of cycles.
     */
    static CycleInstant from_cycles(uint64_t cycles) {
        return CycleInstant(cycles);
    }

    /**
     * @return the number of cycles
     */
    uint64_t as_cycles() const {
        return _cycles;
    }

    /**
     * @return the number of cycles elapsed from another instant to this one.
     */
    CycleDuration duration_since(const CycleInstant &earlier) const {
        assert(_cycles >= earlier._cycles);
        return CycleDuration::from_raw(_cycles - earlier._cycles);
    }

    /**
     * @return the number of cycles elapsed since this instant was created.
     */
    CycleDuration elapsed() const {
        return CycleDuration::from_raw(now()._cycles - _cycles);
    }

    friend CycleInstant operator+(CycleInstant i, const CycleDuration &d) {
        i._cycles += d.as_raw();
        return i;
    }

private:
    uint64_t _cycles;
};

static inline bool operator==(const CycleInstant &lhs, const CycleInstant &rhs) {
    return lhs.as_cycles() == rhs.as_cycles();
}
static inline bool operator!=(const CycleInstant &lhs, const CycleInstant &rhs) {
    return !operator==(lhs, rhs);
}
static inline bool operator<(const CycleInstant &lhs, const CycleInstant &rhs) {
    return lhs.as_cycles() < rhs.as_cycles();
}
static inline bool operator>(const CycleInstant &lhs, const CycleInstant &rhs) {
    return operator<(rhs, lhs);
}
static inline bool operator<=(const CycleInstant &lhs, const CycleInstant &rhs) {
    return !operator>(lhs, rhs);
}
static inline bool operator>=(const CycleInstant &lhs, const CycleInstant &rhs) {
    return !operator<(lhs, rhs);
}

}
