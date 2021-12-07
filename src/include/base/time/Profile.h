/*
 * Copyright (C) 2016-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/Common.h>
#include <base/util/Math.h>
#include <base/stream/OStream.h>
#include <base/time/Instant.h>

#include <memory>

namespace m3 {

template<typename T = CycleDuration>
class Results {
public:
    explicit Results(size_t runs)
        : _runs(0),
          _times(new T[runs]) {
    }

    size_t runs() const {
        return _runs;
    }

    void push(T time) {
        _times[_runs++] = time;
    }

    T avg() const {
        typename T::raw_t sum = 0;
        for(size_t i = 0; i < _runs; ++i)
            sum += _times[i].as_raw();
        return T::from_raw(_runs == 0 ? 0 : sum / _runs);
    }

    T stddev() const {
        typename T::raw_t sum = 0;
        auto average = avg().as_raw();
        for(size_t i = 0; i < _runs; ++i) {
            typename T::raw_t val;
            if(_times[i].as_raw() < average)
                val = average - _times[i].as_raw();
            else
                val = _times[i].as_raw() - average;
            sum += val * val;
        }
        return T::from_raw(_runs == 0 ? 0 : Math::sqrt((float)sum / _runs));
    }

    friend OStream &operator<<(OStream &os, const Results &r) {
        os << r.avg() << " (+/- " << r.stddev() << " with " << r.runs() << " runs)";
        return os;
    }

private:
    size_t _runs;
    std::unique_ptr<T[]> _times;
};

struct Runner {
    virtual ~Runner() {
    }
    virtual void pre() {
    }
    virtual void run() = 0;
    virtual void post() {
    }
};

class Profile {
public:
    explicit Profile(ulong repeats = 100, ulong warmup = 10)
        : _repeats(repeats),
          _warmup(warmup) {
    }

    template<class T, typename F>
    ALWAYS_INLINE Results<typename T::Duration> run(F func) const {
        Results<typename T::Duration> res(_warmup + _repeats);
        for(ulong i = 0; i < _warmup + _repeats; ++i) {
            auto start = T::now();
            func();
            auto end = T::now();

            if(i >= _warmup)
                res.push(end.duration_since(start));
        }
        return res;
    }

    template<class T, class R>
    ALWAYS_INLINE Results<typename T::Duration> runner(R &runner) const {
        Results<typename T::Duration> res(_warmup + _repeats);
        for(ulong i = 0; i < _warmup + _repeats; ++i) {
            runner.pre();

            auto start = T::now();
            runner.run();
            auto end = T::now();

            runner.post();

            if(i >= _warmup)
                res.push(end.duration_since(start));
        }
        return res;
    }

private:
    ulong _repeats;
    ulong _warmup;
};

}
