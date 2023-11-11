/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

#include <base/TCU.h>
#include <base/col/SList.h>
#include <base/util/Util.h>

#include <m3/cap/ObjCap.h>
#include <m3/com/EP.h>

#include <memory>
#include <utility>

namespace m3 {

class GenericFile;
class Syscalls;
class OwnActivity;

/**
 * A lazily activated gate
 *
 * This type exists in two states: unactivated and activated. It can be used via `LazyGate::get`,
 * which will first activate it if not already done and return a usable gate.
 *
 * Lazy activation is normally not necessary and also not desired as it comes with some overhead.
 * However, in some cases a gate needs to be activated lazily, i.e., on first use. For example, if
 * the gate is obtained from somebody else we cannot activate it immediately as the capability does
 * not exist until the obtain operation is finished.
 */
template<class G>
class LazyGate {
public:
    /**
     * Creates a new lazy gate with given capability
     *
     * @param cap the capability
     */
    explicit LazyGate(G::Cap &&cap) : _cap(std::move(cap)), _gate() {
    }

    /**
     * Creates a LazyGate object that is already a gate
     *
     * @param gate the gate to use
     */
    explicit LazyGate(G *gate) : _cap(G::Cap::bind(KIF::INV_SEL)), _gate(gate) {
    }

    ~LazyGate() {
        if(_cap.sel() != KIF::INV_SEL)
            delete _gate;
    }

    /**
     * @return the capability
     */
    G::Cap &cap() noexcept {
        return _cap;
    }

    /**
     * Requests access to the gate and returns a reference to it
     *
     * If not already done, this call will activate the gate.
     *
     * @return the gate
     */
    G &get() noexcept {
        if(!_gate)
            _gate = new G(_cap.activate());
        return *_gate;
    }

private:
    G::Cap _cap;
    G *_gate;
};

/**
 * Gate is the base class of all gates. A gate is in general the software abstraction for TCU-based
 * communication. There are three different kinds of gates: SendGate, RecvGate and MemGate.
 * SendGate and RecvGate allow to perform message-based communication, while MemGate allows to
 * read/write from/to tile-external memory.
 *
 * Before gates can be used, they need to be activated. That is, a syscall needs to be performed to
 * let the kernel configure an endpoint for the gate. For SendGate and MemGate, this is done
 * automatically by EPMng. For RecvGate, it needs to be done manually.
 *
 * On top of Gate, GateStream provides an easy way to marshall/unmarshall data.
 */
class Gate : public ObjCap {
    friend class EPMng;
    friend class RecvGate;
    friend class SendGate;
    friend class GenericFile;
    friend class Syscalls;
    friend class Activity;

public:
    static const epid_t UNBOUND = TOTAL_EPS;

protected:
    explicit Gate(uint type, capsel_t cap, unsigned capflags, EP *ep) noexcept
        : ObjCap(type, cap, capflags),
          _ep(ep) {
    }

    explicit Gate(uint type, capsel_t cap, unsigned capflags, epid_t ep = UNBOUND) noexcept
        : ObjCap(type, cap, capflags),
          _ep(ep == UNBOUND ? nullptr : new EP(EP::bind(ep))) {
    }

public:
    Gate(Gate &&g) noexcept : ObjCap(std::move(g)), _ep(g._ep) {
        g._ep = nullptr;
    }
    virtual ~Gate();

    const EP &activate(capsel_t rbuf_mem = KIF::INV_SEL, goff_t rbuf_off = 0);
    void activate_on(const EP &ep, capsel_t rbuf_mem = KIF::INV_SEL, goff_t rbuf_off = 0);
    void deactivate();

protected:
    const EP *ep() const noexcept {
        return _ep;
    }
    void set_ep(EP *ep) noexcept {
        _ep = ep;
    }

    const EP &acquire_ep();
    void release_ep(bool force_inval = false) noexcept;

private:
    EP *_ep;
};

}
