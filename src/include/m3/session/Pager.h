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

#include <base/Panic.h>

#include <m3/session/ClientSession.h>
#include <m3/com/MemGate.h>
#include <m3/com/SendGate.h>
#include <m3/com/RecvGate.h>

namespace m3 {

class Pager : public ClientSession {
private:
    explicit Pager(VPE &vpe, capsel_t sess)
        : ClientSession(sess),
          _rgate(vpe.pe_desc().has_mmu() ? RecvGate::create_for(vpe, nextlog2<64>::val, nextlog2<64>::val)
                                    : RecvGate::bind(ObjCap::INVALID, 0)),
          _own_sgate(SendGate::bind(obtain(1).start())),
          _child_sgate(SendGate::bind(obtain(1).start())),
          _close(true) {
    }

public:
    enum DelOp {
        DATASPACE,
        MEMGATE,
    };

    enum Operation {
        PAGEFAULT,
        CLONE,
        MAP_ANON,
        UNMAP,
        CLOSE,
        COUNT,
    };

    enum Flags {
        MAP_PRIVATE = 0,
        MAP_SHARED  = 0x2000,
    };

    enum Prot {
        READ    = MemGate::R,
        WRITE   = MemGate::W,
        EXEC    = MemGate::X,
        RW      = READ | WRITE,
        RWX     = READ | WRITE | EXEC,
    };

    explicit Pager(capsel_t sess) noexcept
        : ClientSession(sess),
          _rgate(RecvGate::bind(ObjCap::INVALID, nextlog2<64>::val)),
          _own_sgate(SendGate::bind(obtain(1).start())),
          _child_sgate(SendGate::bind(ObjCap::INVALID)),
          _close(false) {
    }
    explicit Pager(VPE &vpe, const String &service)
        : ClientSession(service),
          _rgate(vpe.pe_desc().has_mmu() ? RecvGate::create_for(vpe, nextlog2<64>::val, nextlog2<64>::val)
                                    : RecvGate::bind(ObjCap::INVALID, 0)),
          _own_sgate(SendGate::bind(obtain(1).start())),
          _child_sgate(SendGate::bind(obtain(1).start())),
          _close(false) {
    }
    ~Pager();

    const SendGate &own_sgate() const noexcept {
        return _own_sgate;
    }
    const SendGate &child_sgate() const noexcept {
        return _child_sgate;
    }

    const RecvGate &child_rgate() const noexcept {
        return _rgate;
    }

    std::unique_ptr<Pager> create_clone(VPE &vpe);
    void clone();
    void pagefault(goff_t addr, uint access);
    void map_anon(goff_t *virt, size_t len, int prot, int flags);
    void map_ds(goff_t *virt, size_t len, int prot, int flags,
                const ClientSession &sess, size_t offset);
    void map_mem(goff_t *virt, MemGate &mem, size_t len, int prot);
    void unmap(goff_t virt);

private:
    RecvGate _rgate;
    SendGate _own_sgate;
    SendGate _child_sgate;
    bool _close;
};

}
