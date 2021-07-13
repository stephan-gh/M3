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

#include <m3/pes/PE.h>
#include <m3/session/ResMng.h>
#include <m3/Exception.h>
#include <m3/Syscalls.h>
#include <m3/pes/VPE.h>

namespace m3 {

constexpr size_t MAX_DESC_LEN = 256;

static PEDesc desc_with_properties(PEDesc desc, const char *props) {
    char props_cpy[MAX_DESC_LEN];
    if(strlen(props) >= MAX_DESC_LEN)
        VTHROW(Errors::NO_SPACE, "PE description too long");
    strcpy(props_cpy, props);

    auto res = desc;
    char *prop = strtok(props_cpy, "+");
    while(prop != nullptr) {
        if(strcmp(prop, "imem") == 0)
            res = PEDesc(PEType::COMP_IMEM, res.isa(), 0);
        else if(strcmp(prop, "emem") == 0 || strcmp(prop, "vm") == 0)
            res = PEDesc(PEType::COMP_EMEM, res.isa(), 0);
        else if(strcmp(prop, "arm") == 0)
            res = PEDesc(res.type(), PEISA::ARM, 0);
        else if(strcmp(prop, "x86") == 0)
            res = PEDesc(res.type(), PEISA::X86, 0);
        else if(strcmp(prop, "riscv") == 0)
            res = PEDesc(res.type(), PEISA::RISCV, 0);
        else if(strcmp(prop, "rocket") == 0)
            res = PEDesc(res.type(), res.isa(), 0, res.attr() | PEAttr::ROCKET);
        else if(strcmp(prop, "boom") == 0)
            res = PEDesc(res.type(), res.isa(), 0, res.attr() | PEAttr::BOOM);
        else if(strcmp(prop, "nic") == 0)
            res = PEDesc(res.type(), res.isa(), 0, res.attr() | PEAttr::NIC);
        else if(strcmp(prop, "kecacc") == 0)
            res = PEDesc(res.type(), res.isa(), 0, res.attr() | PEAttr::KECACC);
        else if(strcmp(prop, "indir") == 0)
            res = PEDesc(PEType::COMP_IMEM, PEISA::ACCEL_INDIR, 0);
        else if(strcmp(prop, "copy") == 0)
            res = PEDesc(PEType::COMP_IMEM, PEISA::ACCEL_COPY, 0);
        else if(strcmp(prop, "rot13") == 0)
            res = PEDesc(PEType::COMP_IMEM, PEISA::ACCEL_ROT13, 0);
        else if(strcmp(prop, "idedev") == 0)
            res = PEDesc(PEType::COMP_IMEM, PEISA::IDE_DEV, 0);
        else if(strcmp(prop, "nicdev") == 0)
            res = PEDesc(PEType::COMP_IMEM, PEISA::NIC_DEV, 0);
        prop = strtok(NULL, "+");
    }
    return res;
}

PE::~PE() {
    if(_free) {
        try {
            VPE::self().resmng()->free_pe(sel());
        }
        catch(...) {
            // ignore
        }
    }
}

Reference<PE> PE::alloc(const PEDesc &desc) {
    capsel_t sel = VPE::self().alloc_sel();
    PEDesc res = VPE::self().resmng()->alloc_pe(sel, desc);
    return Reference<PE>(new PE(sel, res, KEEP_CAP, true));
}

Reference<PE> PE::get(const char *desc) {
    char desc_cpy[MAX_DESC_LEN];
    if(strlen(desc) >= MAX_DESC_LEN)
        VTHROW(Errors::NO_SPACE, "Properties description too long");
    strcpy(desc_cpy, desc);

    auto own = VPE::self().pe();
    char *props = strtok(desc_cpy, "|");
    while(props != nullptr) {
        if(strcmp(props, "own") == 0) {
            if(own->desc().supports_pemux() && own->desc().has_virtmem())
                return own;
        }
        else if(strcmp(props, "clone") == 0) {
            try {
                return PE::alloc(own->desc());
            }
            catch(...) {
            }
        }
        else {
            auto base = PEDesc(own->desc().type(), own->desc().isa(), 0);
            try {
                return PE::alloc(desc_with_properties(base, props));
            }
            catch(...) {
            }
        }
        props = strtok(NULL, "|");
    }
    VTHROW(Errors::NOT_FOUND, "Unable to find PE with " << desc);
}

Reference<PE> PE::derive(uint eps, uint64_t time, uint64_t pts) {
    capsel_t sel = VPE::self().alloc_sel();
    Syscalls::derive_pe(this->sel(), sel, eps, time, pts);
    return Reference<PE>(new PE(sel, desc(), 0, false));
}

void PE::quota(Quota<uint> *eps, Quota<uint64_t> *time, Quota<size_t> *pts) const {
    Syscalls::pe_quota(sel(), eps, time, pts);
}

void PE::set_quota(uint64_t time, uint64_t pts) {
    Syscalls::pe_set_quota(sel(), time, pts);
}

}
