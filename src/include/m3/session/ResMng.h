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

namespace m3 {

class ResMng {
public:
    enum Operation {
        CLONE,
        REG_SERV,
        OPEN_SESS,
        CLOSE_SESS,
    };

    explicit ResMng(capsel_t resmng)
        : _sgate(SendGate::bind(resmng)) {
    }

    capsel_t sel() const {
        return _sgate.sel();
    }
    bool valid() const {
        return _sgate.sel() != ObjCap::INVALID;
    }

    ResMng *clone() const {
        // TODO clone the send gate to the current rmng
        return new ResMng(ObjCap::INVALID);
    }

    Errors::Code register_service(capsel_t dst, capsel_t rgate, const String &name) {
        GateIStream reply = send_receive_vmsg(_sgate, REG_SERV, dst, rgate, name);
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
    SendGate _sgate;
};

}
