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

#include <base/util/Util.h>
#include <base/DTU.h>

#include <m3/com/EP.h>
#include <m3/ObjCap.h>

#include <utility>

namespace m3 {

class GenericFile;
struct RemoteServer;

/**
 * Gate is the base class of all gates. A gate is in general the software abstraction for DTU-based
 * communication. There are three different kinds of gates: SendGate, RecvGate and MemGate.
 * SendGate and RecvGate allow to perform message-based communication, while MemGate allows to
 * read/write from/to PE-external memory.
 *
 * Before gates can be used, they need to be activated. That is, a syscall needs to be performed to
 * let the kernel configure an endpoint for the gate. For SendGate and MemGate, this is done
 * automatically by EPMng. For RecvGate, it needs to be done manually.
 *
 * On top of Gate, GateStream provides an easy way to marshall/unmarshall data.
 */
class Gate : public ObjCap {
    friend class EPMng;
    friend class DTUIf;
    friend class GenericFile;
    friend struct RemoteServer;

public:
    static const epid_t UNBOUND     = EP_COUNT;

protected:
    explicit Gate(uint type, capsel_t cap, unsigned capflags, epid_t ep = UNBOUND) noexcept
        : ObjCap(type, cap, capflags),
          _ep(EP::bind(ep)) {
    }

public:
    Gate(Gate &&g) noexcept
        : ObjCap(std::move(g)),
          _ep(std::move(g._ep)) {
        g._ep.set_id(UNBOUND);
    }
    ~Gate();

protected:
    epid_t ep() const noexcept {
        return _ep.id();
    }
    void set_epid(epid_t id) noexcept {
        _ep.set_id(id);
    }

    void put_ep(EP &&ep, bool assign = true) noexcept;
    EP take_ep() noexcept {
        EP oep = std::move(_ep);
        _ep = EP::bind(UNBOUND);
        return oep;
    }

    epid_t acquire_ep();

private:
    EP _ep;
};

}
