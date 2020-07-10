/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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
#include <base/CPU.h>
#include <string.h>

int memcmp(const void *mem1, const void *mem2, size_t count) {
    size_t align1 = reinterpret_cast<uintptr_t>(mem1) % sizeof(word_t);
    size_t align2 = reinterpret_cast<uintptr_t>(mem2) % sizeof(word_t);
    if(!NEED_ALIGNED_MEMACC || (align1 == 0 && align2 == 0)) {
        const word_t *m1 = reinterpret_cast<const word_t*>(mem1);
        const word_t *m2 = reinterpret_cast<const word_t*>(mem2);
        const word_t *end = m1 + (count / sizeof(word_t));
        while(m1 < end && *m1 == *m2) {
            m1++;
            m2++;
        }
        count -= reinterpret_cast<uintptr_t>(m1) - reinterpret_cast<uintptr_t>(mem1);
        mem1 = m1;
        mem2 = m2;
    }

    const uint8_t *bmem1 = static_cast<const uint8_t*>(mem1);
    const uint8_t *bmem2 = static_cast<const uint8_t*>(mem2);
    for(size_t i = 0; i < count; i++) {
        if(bmem1[i] > bmem2[i])
            return 1;
        else if(bmem1[i] < bmem2[i])
            return -1;
    }
    return 0;
}
