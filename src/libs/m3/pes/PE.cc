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

#include <iostream>
#include <sstream>

namespace m3 {

static PEDesc desc_with_properties(PEDesc desc, const std::string &props) {
    auto res = desc;
    std::stringstream ss(props);
    std::string prop;
    while(std::getline(ss, prop, '+')) {
        if(prop == "imem")
            res = PEDesc(PEType::COMP_IMEM, res.isa(), 0);
        else if(prop == "emem" || prop == "vm")
            res = PEDesc(PEType::COMP_EMEM, res.isa(), 0);
        else if(prop == "arm")
            res = PEDesc(res.type(), PEISA::ARM, 0);
        else if(prop == "x86")
            res = PEDesc(res.type(), PEISA::X86, 0);
        else if(prop == "riscv")
            res = PEDesc(res.type(), PEISA::RISCV, 0);
        else if(prop == "rocket")
            res = PEDesc(res.type(), res.isa(), 0, res.attr() | PEAttr::ROCKET);
        else if(prop == "boom")
            res = PEDesc(res.type(), res.isa(), 0, res.attr() | PEAttr::BOOM);
        else if(prop == "nic")
            res = PEDesc(res.type(), res.isa(), 0, res.attr() | PEAttr::NIC);
        else if(prop == "indir")
            res = PEDesc(PEType::COMP_IMEM, PEISA::ACCEL_INDIR, 0);
        else if(prop == "copy")
            res = PEDesc(PEType::COMP_IMEM, PEISA::ACCEL_COPY, 0);
        else if(prop == "rot13")
            res = PEDesc(PEType::COMP_IMEM, PEISA::ACCEL_ROT13, 0);
        else if(prop == "idedev")
            res = PEDesc(PEType::COMP_IMEM, PEISA::IDE_DEV, 0);
        else if(prop == "nicdev")
            res = PEDesc(PEType::COMP_IMEM, PEISA::NIC_DEV, 0);
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

Reference<PE> PE::get(const std::string &desc) {
    auto own = VPE::self().pe();
    std::stringstream ss(desc);
    std::string props;
    while(std::getline(ss, props, '|')) {
        if(props == "own") {
            if(own->desc().supports_pemux() && own->desc().has_virtmem())
                return own;
        }
        else if(props == "clone") {
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
    }
    VTHROW(Errors::NOT_FOUND, "Unable to find PE with " << desc.c_str());
}

Reference<PE> PE::derive(uint eps) {
    capsel_t sel = VPE::self().alloc_sel();
    Syscalls::derive_pe(this->sel(), sel, eps);
    return Reference<PE>(new PE(sel, desc(), 0, false));
}

uint PE::quota() const {
    return Syscalls::pe_quota(sel());
}

}
