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

template<typename K, typename V, V INVAL, size_t N>
class TinyMap {
    struct Entry {
        K key;
        V val;
    };
public:
    explicit TinyMap()
        : _entries() {
        for(size_t i = 0; i < N; ++i)
            _entries[i].val = INVAL;
    }

    bool insert(K key, V val) {
        for(size_t i = 0; i < N; ++i) {
            if(_entries[i].val == INVAL) {
                _entries[i].key = key;
                _entries[i].val = val;
                return true;
            }
        }
        return false;
    }

    V find(K key) const {
        Entry *e = find_entry(key);
        return e ? e->val : INVAL;
    }

    V remove(K key) {
        Entry *e = find_entry(key);
        if(!e)
            return INVAL;
        return std::swap(e->val, INVAL);
    }

private:
    Entry *find_entry(K key) {
        for(size_t i = 0; i < N; ++i) {
            if(_entries[i].key == key)
                return _entries + i;
        }
        return nullptr;
    }

    Entry _entries[N];
};

}
