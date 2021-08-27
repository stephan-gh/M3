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
#include <base/util/Time.h>

#include <base/stream/OStream.h>

#include <memory>

namespace m3 {

class Profile;

// expects cycles and yields cycles as integer
struct CycleResult {
    using time_t = cycles_t;
    using avg_t = time_t;

    const char *name() const {
        return "cycles";
    }
    time_t get_result(time_t time) const {
        return time;
    }
};

// expects nanoseconds and yields nanoseconds as integer
struct NanoResult {
    using time_t = uint64_t;
    using avg_t = time_t;

    const char *name() const {
        return "ns";
    }
    time_t get_result(time_t time) const {
        return time;
    }
};

// expects nanoseconds and yields microseconds as integer
struct MicroResult {
    using time_t = uint64_t;
    using avg_t = time_t;

    const char *name() const {
        return "us";
    }
    time_t get_result(time_t time) const {
        return time / 1000;
    }
};

// expects nanoseconds and yields milliseconds as float
struct MilliFloatResult {
    using time_t = uint64_t;
    using avg_t = float;

    const char *name() const {
        return "ms";
    }
    float get_result(time_t time) const {
        return time / 1000000.;
    }
};

template<typename T = CycleResult>
class Results : public T {
    friend class Profile;

public:
    using time_t = typename T::time_t;
    using avg_t = typename T::avg_t;

    explicit Results(size_t runs)
        : _runs(0),
          _times(new time_t[runs]) {
    }

    size_t runs() const {
        return _runs;
    }

    avg_t avg() const {
        time_t sum = 0;
        for(size_t i = 0; i < _runs; ++i)
            sum += _times[i];
        return _runs == 0 ? 0 : this->get_result(sum / _runs);
    }

    float stddev() const {
        avg_t sum = 0;
        avg_t average = avg();
        for(size_t i = 0; i < _runs; ++i) {
            size_t val;
            avg_t time_i = this->get_result(_times[i]);
            if(time_i < average)
                val = average - time_i;
            else
                val = time_i - average;
            sum += val * val;
        }
        return _runs == 0 ? 0 : Math::sqrt((float)sum / _runs);
    }

    friend OStream &operator<<(OStream &os, const Results &r) {
        os << r.avg() << " " << r.name()
           << " (+/- " << r.stddev() << " with " << r.runs() << " runs)";
        return os;
    }

    void push(time_t time) {
        _times[_runs++] = time;
    }

private:
    size_t _runs;
    std::unique_ptr<time_t[]> _times;
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

    template<typename F>
    ALWAYS_INLINE Results<> run(F func) const {
        return run_with_id(func, 0);
    }

    template<typename F>
    ALWAYS_INLINE Results<> run_with_id(F func, unsigned id) const {
        Results<> res(_warmup + _repeats);
        for(ulong i = 0; i < _warmup + _repeats; ++i) {
            auto start = Time::start(id);
            func();
            auto end = Time::stop(id);

            if(i >= _warmup)
                res.push(end - start);
        }
        return res;
    }

    template<class R>
    ALWAYS_INLINE Results<> runner_with_id(R &runner, unsigned id) const {
        Results<> res(_warmup + _repeats);
        for(ulong i = 0; i < _warmup + _repeats; ++i) {
            runner.pre();

            auto start = Time::start(id);
            runner.run();
            auto end = Time::stop(id);

            runner.post();

            if(i >= _warmup)
                res.push(end - start);
        }
        return res;
    }

private:
    ulong _repeats;
    ulong _warmup;
};

}
