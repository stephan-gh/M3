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

#include "cap/CapTable.h"
#include "pes/VPE.h"
#include "DTUState.h"

namespace kernel {

class PEMux {
public:
    static const size_t PEXC_MSGSIZE_ORD     = 7;
    // TODO is there a better way?
    static const capsel_t VPE_SEL_BEGIN      = 1000;

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
        return &_pe;
    }
    peid_t peid() const {
        return _pe.id;
    }
    VPEDesc desc() const {
        return VPEDesc(peid(), VPE::INVALID_ID);
    }

    bool used() const {
        return _vpes > 0;
    }
    void add_vpe(VPECapability *vpe) {
        assert(_vpes == 0);
        _caps.obtain(VPE_SEL_BEGIN + vpe->obj->id(), vpe);
        _vpes++;
    }
    void remove_vpe(UNUSED VPE *vpe) {
        // has already been revoked
        assert(_caps.get(VPE_SEL_BEGIN + vpe->id(), Capability::VIRTPE) == nullptr);
        _vpes--;
        _reply_eps = EP_COUNT + 2;
        _rbufs_size = 0;
    }

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

    void set_rbufsize(size_t size) {
        _rbufs_size = size;
    }

    DTUState &dtustate() {
        return _dtustate;
    }

    m3::Errors::Code alloc_ep(VPE *caller, vpeid_t dst, capsel_t sel, epid_t *ep);
    void free_ep(epid_t ep);

    void handle_call(const m3::DTU::Message *msg);

    void pexcall_activate(const m3::DTU::Message *msg);

    size_t allocate_reply_eps(size_t num);

    bool invalidate_ep(epid_t ep, bool force = false);
    void invalidate_eps();

    m3::Errors::Code config_rcv_ep(epid_t ep, RGateObject &obj);
    m3::Errors::Code config_snd_ep(epid_t ep, SGateObject &obj);
    m3::Errors::Code config_mem_ep(epid_t ep, const MGateObject &obj, goff_t off);
    void update_ep(epid_t ep);

private:
    PEObject _pe;
    CapTable _caps;
    size_t _vpes;
    size_t _reply_eps;
    size_t _rbufs_size;
    goff_t _mem_base;
    DTUState _dtustate;
    SendQueue _upcqueue;
};

}
