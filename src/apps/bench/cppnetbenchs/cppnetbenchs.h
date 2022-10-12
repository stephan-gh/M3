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

#include <base/stream/Format.h>
#include <base/time/Profile.h>

#define RUN_SUITE(name)                                  \
    m3::println("Running benchmark suite {}"_cf, #name); \
    name();                                              \
    m3::println();

#define RUN_BENCH(name)                                       \
    m3::println("Testing \"{}\" in {}:"_cf, #name, __FILE__); \
    name();                                                   \
    m3::println();

template<typename T>
class MilliFloatResultRef {
public:
    explicit MilliFloatResultRef(const m3::Results<T> &res) : _res(res) {
    }

    const m3::Results<T> &_res;
};

template<typename T>
struct m3::Formatter<MilliFloatResultRef<T>> {
    template<typename O>
    constexpr void format(O &out, UNUSED const FormatSpecs &specs,
                          const MilliFloatResultRef<T> &r) const {
        using namespace m3;
        format_to(out, "{} ms (+/- {} ms with {} runs)"_cf,
                  static_cast<float>(r._res.avg().as_nanos()) / 1000000.f,
                  static_cast<float>(r._res.stddev().as_nanos()) / 1000000.f, r._res.runs());
    }
};

void budp();
void btcp();
