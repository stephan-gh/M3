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

#include <base/time/Profile.h>

#define RUN_SUITE(name)                                                 \
    m3::cout << "Running benchmark suite " << #name << " ...\n";        \
    name();                                                             \
    m3::cout << "\n";

#define RUN_BENCH(name)                                                 \
    m3::cout << "Testing \"" << #name << "\" in " << __FILE__ << ":\n"; \
    name();                                                             \
    m3::cout << "\n";

template<typename T>
class MilliFloatResultRef {
public:
    explicit MilliFloatResultRef(const m3::Results<T> &res) : _res(res) {
    }

    friend m3::OStream &operator<<(m3::OStream &os, const MilliFloatResultRef &r) {
        os << (static_cast<float>(r._res.avg().as_nanos()) / 1000000.f)
           << " ms (+/- " << (static_cast<float>(r._res.stddev().as_nanos()) / 1000000.f)
           << " ms with " << r._res.runs() << " runs)";
        return os;
    }

    const m3::Results<T> &_res;
};

void budp();
void btcp();
