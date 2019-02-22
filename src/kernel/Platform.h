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

#include <base/PEDesc.h>

namespace kernel {

class Platform {
public:
    struct BootModule {
        uint64_t addr;
        uint64_t size;
        uint64_t namelen;
        char name[];
    } PACKED;

    class BootModuleIterator {
    public:
        explicit BootModuleIterator(BootModule *mod = nullptr) : _mod(mod) {
        }

        BootModule & operator*() const {
            return *this->_mod;
        }
        BootModule *operator->() const {
            return &operator*();
        }
        BootModuleIterator& operator++() {
            uintptr_t next = reinterpret_cast<uintptr_t>(_mod) + sizeof(BootModule) + _mod->namelen;
            _mod = reinterpret_cast<BootModule*>(next);
            return *this;
        }
        BootModuleIterator operator++(int) {
            BootModuleIterator tmp(*this);
            operator++();
            return tmp;
        }
        bool operator==(const BootModuleIterator& rhs) const {
            return _mod == rhs._mod;
        }
        bool operator!=(const BootModuleIterator& rhs) const {
            return _mod != rhs._mod;
        }

    private:
        BootModule *_mod;
    };

    struct KEnv {
        explicit KEnv();

        uint64_t mod_count;
        uint64_t mod_size;
        uint64_t pe_count;
    } PACKED;

    static peid_t kernel_pe();
    static peid_t first_pe();
    static peid_t last_pe();

    static BootModuleIterator mods_begin() {
        return BootModuleIterator(_mods);
    }
    static BootModuleIterator mods_end() {
        uintptr_t last = reinterpret_cast<uintptr_t>(_mods) + _kenv.mod_size;
        return BootModuleIterator(reinterpret_cast<BootModule*>(last));
    }

    static size_t pe_count() {
        return _kenv.pe_count;
    }
    static m3::PEDesc pe(peid_t no) {
        return _pes[no];
    }

    static goff_t def_recvbuf(peid_t no);

private:
    static m3::PEDesc *_pes;
    static BootModule *_mods;
    static KEnv _kenv;
};

}
