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

#include <base/mem/AreaManager.h>
#include <base/GlobAddr.h>

#include "Types.h"

namespace kernel {

struct MemoryArea : public m3::Area {
    static const size_t MAX_AREAS   = 4096;

    static void init();

    static void *operator new(size_t);
    static void operator delete(void *ptr);

    static MemoryArea *freelist;
    static MemoryArea areas[MAX_AREAS];
};

class MemoryModule {
public:
    enum Type {
        KERNEL,
        USER,
        OCCUPIED,
    };

    explicit MemoryModule(Type type, m3::GlobAddr addr, size_t size)
        : _type(type),
           _addr(addr),
           _size(size),
           _map(addr.offset(), size) {
    }

    Type type() const {
        return _type;
    }
    peid_t pe() const {
        return _addr.pe();
    }
    m3::GlobAddr addr() const {
        return _addr;
    }
    size_t size() const {
        return _size;
    }
    m3::AreaManager<MemoryArea> &map() {
        return _map;
    }

private:
    Type _type;
    m3::GlobAddr _addr;
    size_t _size;
    m3::AreaManager<MemoryArea> _map;
};

}
