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
    static const size_t MAX_MEMS = 4;

    struct PE {
        uint32_t id;
        PEDesc desc;
    } PACKED;

    struct Mod {
        uint64_t addr;
        uint64_t size;
        uint64_t namelen;
        char name[];
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

    class ModIterator {
    public:
        explicit ModIterator(Mod *mod = nullptr) : _mod(mod) {
        }

        Mod & operator*() const {
            return *this->_mod;
        }
        Mod *operator->() const {
            return &operator*();
        }
        ModIterator& operator++() {
            uintptr_t next = reinterpret_cast<uintptr_t>(_mod) + sizeof(Mod) + _mod->namelen;
            _mod = reinterpret_cast<Mod*>(next);
            return *this;
        }
        ModIterator operator++(int) {
            ModIterator tmp(*this);
            operator++();
            return tmp;
        }
        bool operator==(const ModIterator& rhs) const {
            return _mod == rhs._mod;
        }
        bool operator!=(const ModIterator& rhs) const {
            return _mod != rhs._mod;
        }

    private:
        Mod *_mod;
    };

    uint64_t mod_count;
    uint64_t mod_size;
    uint64_t pe_count;
    Mem mems[MAX_MEMS];
} PACKED;

}
