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

#include <base/BootInfo.h>
#include <base/PEDesc.h>

#include "Types.h"

namespace kernel {

class Platform {
public:
    static void init();
    static void add_modules(int argc, char **argv);

    static peid_t kernel_pe();
    static peid_t first_pe();
    static peid_t last_pe();

    static m3::BootInfo::ModIterator mods_begin() {
        return m3::BootInfo::ModIterator(_mods);
    }
    static m3::BootInfo::ModIterator mods_end() {
        uintptr_t last = reinterpret_cast<uintptr_t>(_mods) + _info.mod_size;
        return m3::BootInfo::ModIterator(reinterpret_cast<m3::BootInfo::Mod*>(last));
    }

    static gaddr_t info_addr();
    static size_t info_size() {
        return sizeof(_info) + _info.mod_size + _info.pe_count * sizeof(m3::PEDesc);
    }
    static size_t pe_count() {
        return _info.pe_count;
    }
    static size_t mod_count() {
        return _info.mod_count;
    }
    static m3::PEDesc pe(peid_t no) {
        return _pes[no];
    }

    static goff_t rbuf_pemux(peid_t no);
    static goff_t rbuf_std(peid_t no, vpeid_t vpe);

    static bool is_shared(peid_t no);

private:
    static m3::PEDesc *_pes;
    static m3::BootInfo::Mod *_mods;
    static m3::BootInfo _info;
};

}
