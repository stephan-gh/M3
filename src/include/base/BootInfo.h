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

#pragma once

#include <base/Common.h>
#include <base/PEDesc.h>

#include <assert.h>

namespace m3 {

class BootInfo {
public:
    static const size_t MAX_MODNAME_LEN     = 64;
    static const size_t MAX_SERVNAME_LEN    = 32;

    struct PE {
        uint32_t id;
        PEDesc desc;
    } PACKED;

    struct Mod {
        uint64_t addr;
        uint64_t size;
        char name[MAX_MODNAME_LEN];
    } PACKED;

    class Mem {
    public:
        explicit Mem()
            : _addr(), _size() {
        }
        explicit Mem(uint64_t addr, uint64_t size, bool reserved)
            : _addr(addr), _size(size | (reserved ? 1 : 0)) {
            assert((size & 1) == 0);
        }

        uint64_t addr() const {
            return _addr;
        }
        uint64_t size() const {
            return _size & ~static_cast<uint64_t>(1);
        }
        bool reserved() const {
            return (_size & 1) == 1;
        }

    private:
        uint64_t _addr;
        uint64_t _size;
    } PACKED;

    struct Service {
        uint32_t sessions;
        char name[MAX_SERVNAME_LEN];
    } PACKED;

    uint64_t mod_count;
    uint64_t pe_count;
    uint64_t mem_count;
    uint64_t serv_count;
} PACKED;

}
