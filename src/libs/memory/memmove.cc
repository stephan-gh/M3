/*
 * Copyright (C) 2015-2016, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

/* this is necessary to prevent that gcc transforms a loop into library-calls
 * (which might lead to recursion here) */
#pragma GCC optimize ("no-tree-loop-distribute-patterns")

void *memmove(void *dest, const void *src, size_t count) {
    /* nothing to do? */
    if(reinterpret_cast<uint8_t*>(dest) == reinterpret_cast<const uint8_t*>(src))
        return dest;

    const uint8_t *s = reinterpret_cast<const uint8_t*>(src);
    uint8_t *d = reinterpret_cast<uint8_t*>(dest);

    // move backwards if they overlap
    if(s < d && d < s + count) {
        s += count;
        d += count;

        // copy words, if possible
        if(static_cast<size_t>(d - s) >= sizeof(word_t)) {
            size_t dalign = reinterpret_cast<uintptr_t>(d) % sizeof(word_t);
            size_t salign = reinterpret_cast<uintptr_t>(s) % sizeof(word_t);
            if(!NEED_ALIGNED_MEMACC || (dalign == 0 && salign == 0)) {
                // copy words
                word_t *ddest = reinterpret_cast<word_t*>(d);
                const word_t *dsrc = reinterpret_cast<const word_t*>(s);
                while(count >= sizeof(word_t)) {
                    *--ddest = *--dsrc;
                    count -= sizeof(word_t);
                }

                d = reinterpret_cast<uint8_t*>(ddest);
                s = reinterpret_cast<const uint8_t*>(dsrc);
            }
        }

        // copy remaining bytes
        while(count-- > 0)
            *--d = *--s;
    }
    // move forward
    else
        memcpy(dest, src, count);

    return dest;
}
