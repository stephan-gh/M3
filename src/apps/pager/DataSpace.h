/*
 * Copyright (C) 2015, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <m3/Common.h>
#include <m3/col/Treap.h>
#include <m3/cap/MemGate.h>
#include <m3/cap/Session.h>
#include <m3/stream/OStream.h>
#include <m3/service/M3FS.h>
#include <m3/CapRngDesc.h>
#include <m3/DTU.h>
#include <m3/Errors.h>

#include "RegionList.h"

namespace m3 {

class DataSpace : public TreapNode<uintptr_t> {
public:
    explicit DataSpace(uintptr_t addr, size_t size, uint _flags)
        : TreapNode<uintptr_t>(addr), flags(_flags), _regs(size), _size(size) {
    }

    bool matches(uintptr_t k) override {
        return k >= addr() && k < addr() + _size;
    }

    uintptr_t addr() const {
        return key();
    }
    size_t size() const {
        return _size;
    }

    void print(OStream &os) const override {
        os << "DataSpace[addr=" << fmt(addr(), "p") << ", size=" << fmt(size(), "#x")
           << ", flags=" << flags << "]";
    }

    virtual Errors::Code get_page(uintptr_t *virt, int *pageNo, size_t *pages, capsel_t *sel) = 0;

    uint flags;
protected:
    RegionList _regs;
    size_t _size;
};

class AnonDataSpace : public DataSpace {
public:
    static constexpr size_t MAX_PAGES = 16;

    explicit AnonDataSpace(uintptr_t addr, size_t size, uint flags)
        : DataSpace(addr, size, flags) {
    }

    Errors::Code get_page(uintptr_t *virt, int *pageNo, size_t *pages, capsel_t *sel) override {
        Region *reg = _regs.pagefault(*virt - addr());
        if(reg->mem() != NULL) {
            // TODO don't assume that memory is never unmapped.
            *pages = 0;
            return Errors::NO_ERROR;
        }

        reg->size(Math::min(reg->size(), MAX_PAGES * PAGE_SIZE));
        reg->mem(new MemGate(MemGate::create_global(reg->size(), flags)));
        *pageNo = 0;
        *pages = reg->size() >> PAGE_BITS;
        *sel = reg->mem()->sel();
        return Errors::NO_ERROR;
    }
};

class ExternalDataSpace : public DataSpace {
public:
    explicit ExternalDataSpace(uintptr_t addr, size_t size, uint flags, int _id, size_t _offset)
        : DataSpace(addr, size, flags), sess(VPE::self().alloc_cap()), id(_id), offset(_offset) {
    }

    Errors::Code get_page(uintptr_t *virt, int *pageNo, size_t *pages, capsel_t *sel) override {
        // find the region
        Region *reg = _regs.pagefault(*virt - addr());
        if(reg->mem() != NULL) {
            // TODO don't assume that memory is never unmapped.
            *pages = 0;
            return Errors::NO_ERROR;
        }

        // get memory caps for the region
        auto args = create_vmsg(id, offset + reg->offset(), 1, 0, M3FS::BYTE_OFFSET);
        CapRngDesc crd;
        loclist_type locs;
        GateIStream resp = sess.obtain(1, crd, args);
        if(Errors::last != Errors::NO_ERROR)
            return Errors::last;

        // adjust region
        resp >> locs;
        reg->size(Math::round_up(locs.get(0), PAGE_SIZE));
        reg->mem(new MemGate(MemGate::bind(crd.start())));

        // that's what we want to map
        *pageNo = 0;
        *pages = reg->size() >> PAGE_BITS;
        *virt = addr() + reg->offset();
        *sel = crd.start();
        return Errors::NO_ERROR;
    }

    Session sess;
    int id;
    size_t offset;
};

}
