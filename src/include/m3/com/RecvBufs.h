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

#include <base/mem/AreaManager.h>

#include <m3/com/MemGate.h>
#include <m3/pes/VPE.h>

#include <memory>
#include <utility>

namespace m3 {

class RecvBuf {
public:
    explicit RecvBuf(uintptr_t addr, size_t size, std::unique_ptr<MemGate> &mem)
        : _addr(addr),
          _size(size),
          _mem() {
        _mem.reset(mem.release());
    }

    uintptr_t addr() const {
        return _addr;
    }
    size_t size() const {
        return _size;
    }
    goff_t off() const {
        return _mem ? 0 : _addr;
    }
    capsel_t mem() const {
        return _mem ? _mem->sel() : VPE::self().mem().sel();
    }

private:
    uintptr_t _addr;
    size_t _size;
    std::unique_ptr<MemGate> _mem;
};

class RecvBufs {
    explicit RecvBufs() : _bufs(VPE::self().pe_desc().rbuf_space()) {
    }

public:
    static RecvBufs &get() {
        return _inst;
    }

    RecvBuf *alloc(size_t size);
    void free(RecvBuf *rbuf) noexcept;

private:
    AreaManager<> _bufs;
    static RecvBufs _inst;
};

}
