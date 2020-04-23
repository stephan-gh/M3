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

#include <base/GlobAddr.h>

#include "mem/MemoryModule.h"

namespace m3 {
    class OStream;
}

namespace kernel {

class MainMemory {
    explicit MainMemory();

    static const size_t MAX_MODS    = 4;

public:
    struct Allocation {
        explicit Allocation()
            : mod(),
              offset(),
              size() {
        }
        explicit Allocation(size_t _mod, goff_t _offset, size_t _size)
            : mod(_mod),
              offset(_offset),
              size(_size) {
        }

        operator bool() const {
            return size > 0;
        }
        m3::GlobAddr addr() const {
            return m3::GlobAddr(get().module(mod).pe(), offset);
        }

        size_t mod;
        goff_t offset;
        size_t size;
    };

    static void init() {
        MemoryArea::init();
    }

    static MainMemory &get() {
        return _inst;
    }

    void add(MemoryModule *mod);

    size_t mod_count() const {
        return _count;
    }
    const MemoryModule &module(size_t id) const;
    Allocation build_allocation(m3::GlobAddr global, size_t size) const;

    Allocation allocate(size_t size, size_t align);

    void free(m3::GlobAddr global, size_t size);
    void free(const Allocation &alloc);

    size_t size() const;
    size_t available() const;

    friend m3::OStream &operator<<(m3::OStream &os, const MainMemory &mem);

private:
    size_t _count;
    MemoryModule *_mods[MAX_MODS];
    static MainMemory _inst;
};

}
