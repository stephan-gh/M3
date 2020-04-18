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

#include <base/Panic.h>

#include "mem/MemoryModule.h"

namespace kernel {

MemoryArea *MemoryArea::freelist = nullptr;
MemoryArea MemoryArea::areas[MemoryArea::MAX_AREAS];

void MemoryArea::init() {
    for(size_t i = 0; i < MAX_AREAS; ++i) {
        areas[i].next = freelist;
        freelist = areas + i;
    }
}

void *MemoryArea::operator new(size_t) {
    if(freelist == nullptr)
        PANIC("No free areas");

    void *res = freelist;
    freelist = static_cast<MemoryArea*>(freelist->next);
    return res;
}

void MemoryArea::operator delete(void *ptr) {
    Area *a = static_cast<Area*>(ptr);
    a->next = freelist;
    freelist = static_cast<MemoryArea*>(a);
}

}
