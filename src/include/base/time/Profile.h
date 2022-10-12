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

#include <base/Common.h>
#include <base/stream/OStream.h>
#include <base/time/Instant.h>
#include <base/util/Math.h>

#include <memory>

namespace m3 {

/**
 * Holds the results of time measurements, represented as `T`.
 */
template<typename T = CycleDuration>
class Results {
public:
    /**
     * Creates a new instance with room for the given number of runs.
     *
     * @param runs the maximum number of runs that can be measured
     */
    explicit Results(size_t runs) : _runs(0), _times(new T[runs]) {
    }

    /**
     * @return the number of runs performed so far
     */
    size_t runs() const {
        return _runs;
    }

    /**
     * Pushes the given time to the results. Assumes that there is still room for another time
     * measurement.
     *
     * @param time the measured time
     */
    void push(T time) {
        _times[_runs++] = time;
    }

    /**
     * @return the average time of all measurements
     */
    T avg() const {
        typename T::raw_t sum = 0;
        for(size_t i = 0; i < _runs; ++i)
            sum += _times[i].as_raw();
        return T::from_raw(_runs == 0 ? 0 : sum / _runs);
    }

    /**
     * @return the standard deviation of all measurements
     */
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

    void format(OStream &os, const FormatSpecs &) const {
        format_to(os, "{} (+/- {} with {} runs)"_cf, avg(), stddev(), runs());
    }

private:
    size_t _runs;
    std::unique_ptr<T[]> _times;
};

/**
 * The base class for all runners. Runners are used to support custom actions before and after every
 * benchmark run.
 */
struct Runner {
    virtual ~Runner() {
    }

    /**
     * Performs actions before every benchmark run
     */
    virtual void pre() {
    }

    /**
     * Executes the actual benchmark
     */
    virtual void run() = 0;

    /**
     * Performs actions after every benchmark run
     */
    virtual void post() {
    }
};

/**
 * A profiler performs a specified benchmark a number of times after a warmup phase and returns the
 * results in form of a Result object. Both the number of runs and the number of warmup runs can
 * be customized.
 *
 * Usage example:
 * <code>
 * Profile pr(50, 5);
 * WVPERF("my benchmark", pr.run<CycleInstant>([] {
 *     // my benchmark
 * }));
 * </code>
 */
class Profile {
public:
    /**
     * Creates a new profiler with given number of runs and warmup runs.
     *
     * @param repeats the number of runs (100 by default)
     * @param warmup the number of warmup runs (10 by default)
     */
    explicit Profile(ulong repeats = 100, ulong warmup = 10) : _repeats(repeats), _warmup(warmup) {
    }

    /**
     * Calls the given function as many times as requested for this Profiler object and returns
     * the results.
     *
     * The template parameter T defines the Instant to use for time measurements (e.g., CycleInstant
     * or TimeInstant).
     *
     * @param func the function to call
     * @return the collected results (ignoring warmup runs)
     */
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

    /**
     * Uses the given runner as many times as requested for this Profiler object and returns the
     * results. Before each measurement, runner.pre() is called, whereas after each measurement,
     * runner.post() is called. During the measurement, runner.run() is called.
     *
     * The template parameter T defines the Instant to use for time measurements (e.g., CycleInstant
     * or TimeInstant).
     *
     * @param runner the runner to use
     * @return the collected results (ignoring warmup runs)
     */
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
