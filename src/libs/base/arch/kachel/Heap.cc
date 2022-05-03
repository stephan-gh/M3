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

#include <base/Common.h>
#include <base/Config.h>
#include <base/Env.h>
#include <base/mem/Heap.h>
#include <base/util/Math.h>

#include <assert.h>

extern void *_bss_end;

namespace m3 {

void Heap::init_arch() {
    uintptr_t begin = Math::round_up<uintptr_t>(reinterpret_cast<uintptr_t>(&_bss_end), PAGE_SIZE);

    uintptr_t end;
    if(TileDesc(env()->tile_desc).has_memory())
        end = TileDesc(env()->tile_desc).stack_space().first;
    else {
        // assert(env()->heap_size != 0);
        end = begin + env()->heap_size;
    }

    heap_init(begin, end);
}

}
