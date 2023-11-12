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

#include <base/Panic.h>
#include <base/util/Reference.h>

#include <m3/com/MemGate.h>
#include <m3/com/RecvGate.h>
#include <m3/com/SendGate.h>
#include <m3/session/ClientSession.h>

namespace m3 {

class ChildActivity;

class Pager : public RefCounted, public ClientSession {
private:
    explicit Pager(capsel_t sess);

public:
    enum Flags {
        MAP_PRIVATE = 0,
        MAP_SHARED = 0x2000,
        MAP_UNINIT = 0x4000,
        MAP_NOLPAGE = 0x8000,
    };

    enum Prot {
        READ = MemGate::R,
        WRITE = MemGate::W,
        EXEC = MemGate::X,
        RW = READ | WRITE,
        RWX = READ | WRITE | EXEC,
    };

    explicit Pager(capsel_t sess, capsel_t sgate);

    capsel_t child_sgate() const noexcept {
        return _child_sgate;
    }

    void init(ChildActivity &act);

    Reference<Pager> create_clone();
    void clone();
    void pagefault(goff_t addr, uint access);
    void map_anon(goff_t *virt, size_t len, int prot, int flags);
    void map_ds(goff_t *virt, size_t len, int prot, int flags, const ClientSession &sess,
                size_t offset);
    void map_mem(goff_t *virt, capsel_t mem, size_t len, int prot);
    void unmap(goff_t virt);

private:
    SendGate _req_sgate;
    capsel_t _child_sgate;
    RecvCap _pf_rgate;
    SendCap _pf_sgate;
};

}
