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

#include <assert.h>
#include <utility>

namespace m3 {

template<typename T>
class DefaultUsedPolicy {
public:
    void init(T &v) {
        v = T();
    }
    bool is_used(T &v) const {
        return v != T();
    }
};

template<typename T, size_t N, class U = DefaultUsedPolicy<T>>
class Array : public U {
public:
    explicit Array() {
        for(size_t i = 0; i < N; ++i)
            this->init(_entries[i]);
    }

    size_t find(T val) const {
        for(size_t i = 0; i < N; ++i) {
            if(_entries[i] == val)
                return i;
        }
        return N;
    }

    size_t insert(T val) {
        for(size_t i = 0; i < N; ++i) {
            if(!this->is_used(_entries[i])) {
                _entries[i] = val;
                return i;
            }
        }
        return N;
    }

    size_t remove(T val) {
        size_t i = find(val);
        if(i != N)
            remove_at(i);
        return i;
    }
    void remove_at(size_t idx) {
        this->init(_entries[idx]);
    }

private:
    T _entries[N];
};

}
