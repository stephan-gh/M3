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

#include <base/util/BitField.h>
#include <base/Config.h>

#include "cap/CapTable.h"
#include "Platform.h"

namespace kernel {

class PEMux {
public:
    static const size_t PEXC_MSGSIZE_ORD     = 7;

    static size_t total_instances() {
        size_t num = 0;
        for(peid_t pe = Platform::first_pe(); pe <= Platform::last_pe(); ++pe) {
            if(Platform::pe(pe).is_programmable())
                num++;
        }
        return num;
    }

    explicit PEMux(peid_t pe);

    PEObject *pe() {
        return &*_pe;
    }
    peid_t peid() const {
        return _pe->id;
    }

    void add_vpe(VPECapability *vpe);
    void remove_vpe(VPE *vpe);

    goff_t mem_base() const {
        return _mem_base;
    }
    goff_t eps_base() const {
        return mem_base();
    }
    goff_t rbuf_base() const {
        return mem_base() + EPMEM_SIZE;
    }
    void set_mem_base(goff_t addr) {
        _mem_base = addr;
    }

    epid_t find_eps(uint count) const;
    bool eps_free(epid_t start, uint count) const;
    void alloc_eps(epid_t first, uint count);
    void free_eps(epid_t first, uint count);

    void handle_call(const m3::TCU::Message *msg);

    m3::Errors::Code map(vpeid_t vpe, goff_t virt, gaddr_t phys, uint pages, uint perm);
    m3::Errors::Code vpe_ctrl(VPE *vpe, m3::KIF::PEXUpcalls::VPEOp ctrl);

    m3::Errors::Code invalidate_ep(vpeid_t vpe, epid_t ep, bool force = false);

    m3::Errors::Code config_rcv_ep(epid_t ep, vpeid_t vpe, epid_t rpleps,
                                   RGateObject &obj, bool std = false);
    m3::Errors::Code config_snd_ep(epid_t ep, vpeid_t vpe, SGateObject &obj);
    m3::Errors::Code config_mem_ep(epid_t ep, vpeid_t vpe, const MGateObject &obj, goff_t off);

private:
    m3::Errors::Code upcall(void *req, size_t size);

    m3::Reference<PEObject> _pe;
    CapTable _caps;
    size_t _vpes;
    goff_t _mem_base;
    m3::BitField<EP_COUNT> _eps;
    SendQueue _upcqueue;
};

}
