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

#include <base/Common.h>
#include <base/util/Math.h>
#include <base/Config.h>
#include <base/Heap.h>
#include <base/Env.h>

extern void *_bss_end;

namespace m3 {

void Heap::init_arch() {
    uintptr_t begin = Math::round_up<uintptr_t>(reinterpret_cast<uintptr_t>(&_bss_end), LPAGE_SIZE);

    uintptr_t end;
    if(env()->heap_size == 0) {
        if(PEDesc(env()->pe_desc).has_memory())
            end = PEDesc(env()->pe_desc).mem_size() - RECVBUF_SIZE_SPM;
        // this does only exist so that we can still run scenarios on cache-PEs without pager
        else
            end = begin + ROOT_HEAP_SIZE;
    }
    else
        end = begin + env()->heap_size;

    heap_init(begin, end);
}

}
