/*
 * Copyright (C) 2021 Nils Asmussen, Barkhausen Institut
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

EXTERN_C void gem5_writefile(const char *str, uint64_t len, uint64_t offset, uint64_t file) {
    register word_t r0 asm("r0") = reinterpret_cast<word_t>(str);
    register word_t r1 asm("r1") = 0;
    register word_t r2 asm("r2") = len & 0xFFFF'FFFF;
    register word_t r3 asm("r3") = len >> 32;
    register word_t r4 asm("r4") = offset & 0xFFFF'FFFF;
    register word_t r5 asm("r5") = offset >> 32;
    register word_t r6 asm("r6") = file & 0xFFFF'FFFF;
    register word_t r7 asm("r7") = file >> 32;
    asm volatile(".long 0xEE4F0110"
                 :
                 : "r"(r0), "r"(r1), "r"(r2), "r"(r3), "r"(r4), "r"(r5), "r"(r6), "r"(r7));
}

EXTERN_C ssize_t gem5_readfile(char *dst, uint64_t max, uint64_t offset) {
    register word_t r0 asm("r0") = reinterpret_cast<word_t>(dst);
    register word_t r1 asm("r1") = 0;
    register word_t r2 asm("r2") = max & 0xFFFF'FFFF;
    register word_t r3 asm("r3") = max >> 32;
    register word_t r4 asm("r4") = offset & 0xFFFF'FFFF;
    register word_t r5 asm("r5") = offset >> 32;
    asm volatile(".long 0xEE500110" : "+r"(r0) : "r"(r1), "r"(r2), "r"(r3), "r"(r4), "r"(r5));
    uint64_t res = static_cast<uint64_t>(r0) | static_cast<uint64_t>(r1) << 32;
    return static_cast<ssize_t>(res);
}
