/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019 Nils Asmussen, Barkhausen Institut
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

#include <base/Errors.h>

#include <m3/com/GateStream.h>
#include <m3/com/SendGate.h>
#include <m3/session/ClientSession.h>

namespace m3 {

class Plasma : public ClientSession {
public:
    enum Operation {
        LEFT,
        RIGHT,
        COLUP,
        COLDOWN,
        COUNT
    };

    explicit Plasma(const String &service)
        : ClientSession(service),
          _gate(SendGate::bind(obtain(1).start())) {
    }

    void left() {
        execute(LEFT);
    }
    void right() {
        execute(RIGHT);
    }
    void colup() {
        execute(COLUP);
    }
    void coldown() {
        execute(COLDOWN);
    }

private:
    void execute(Operation op) {
        GateIStream reply = send_receive_vmsg(_gate, op);
        reply.pull_result();
    }

    SendGate _gate;
};

}
