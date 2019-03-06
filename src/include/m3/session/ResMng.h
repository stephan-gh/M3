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

#include <base/Panic.h>

#include <m3/com/GateStream.h>
#include <m3/com/SendGate.h>
#include <m3/VPE.h>

namespace m3 {

class ResMng {
    explicit ResMng(capsel_t resmng, capsel_t vpe)
        : _sgate(SendGate::bind(resmng)), _vpe(vpe) {
    }

public:
    enum Operation {
        REG_SERV,
        UNREG_SERV,

        OPEN_SESS,
        CLOSE_SESS,

        ADD_CHILD,
        REM_CHILD,
    };

    explicit ResMng(capsel_t resmng)
        : _sgate(SendGate::bind(resmng)), _vpe(ObjCap::INVALID) {
    }
    ~ResMng() {
        if(_vpe != ObjCap::INVALID)
            send_receive_vmsg(VPE::self().resmng()._sgate, REM_CHILD, _vpe);
    }

    capsel_t sel() const {
        return _sgate.sel();
    }
    bool valid() const {
        return _sgate.sel() != ObjCap::INVALID;
    }

    ResMng *clone(VPE &vpe, const String &name) {
        capsel_t sgate_sel = vpe.alloc_sel();
        Errors::Code res = clone(vpe.sel(), sgate_sel, name);
        if(res != Errors::NONE)
            return nullptr;
        return new ResMng(sgate_sel, vpe.sel());
    }

    Errors::Code reg_service(capsel_t child, capsel_t dst, capsel_t rgate, const String &name) {
        GateIStream reply = send_receive_vmsg(_sgate, REG_SERV, child, dst, rgate, name);
        Errors::Code res;
        reply >> res;
        return res;
    }

    Errors::Code unreg_service(capsel_t sel, bool notify) {
        GateIStream reply = send_receive_vmsg(_sgate, UNREG_SERV, sel, notify);
        Errors::Code res;
        reply >> res;
        return res;
    }

    Errors::Code open_sess(capsel_t dst, const String &name, uint64_t arg = 0) {
        GateIStream reply = send_receive_vmsg(_sgate, OPEN_SESS, dst, name, arg);
        Errors::Code res;
        reply >> res;
        return res;
    }

    Errors::Code close_sess(capsel_t sel) {
        GateIStream reply = send_receive_vmsg(_sgate, CLOSE_SESS, sel);
        Errors::Code res;
        reply >> res;
        return res;
    }

private:
    Errors::Code clone(capsel_t vpe_sel, capsel_t sgate_sel, const String &name) {
        GateIStream reply = send_receive_vmsg(_sgate, ADD_CHILD, vpe_sel, sgate_sel, name);
        Errors::Code res;
        reply >> res;
        return res;
    }

    SendGate _sgate;
    capsel_t _vpe;
};

}
