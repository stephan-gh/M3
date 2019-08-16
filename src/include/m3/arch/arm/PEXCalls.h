/*
 * Copyright (C) 2016, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/Common.h>
#include <base/Errors.h>

namespace m3 {

class PEXCalls {
public:
    static word_t call1(Operation op, word_t arg1) {
        register word_t r0 asm("r0") = op;
        register word_t r1 asm("r1") = arg1;
        asm volatile(
            "svc $0"
            : "+r"(r0)
            : "r"(r1)
            : "memory"
        );
        return r0;
    }

    static word_t call2(Operation op, word_t arg1, word_t arg2) {
        register word_t r0 asm("r0") = op;
        register word_t r1 asm("r1") = arg1;
        register word_t r2 asm("r2") = arg2;
        asm volatile(
            "svc $0"
            : "+r"(r0)
            : "r"(r1), "r"(r2)
            : "memory"
        );
        return r0;
    }

    static word_t call4(Operation op, word_t arg1, word_t arg2, word_t arg3, word_t arg4) {
        register word_t r0 asm("r0") = op;
        register word_t r1 asm("r1") = arg1;
        register word_t r2 asm("r2") = arg2;
        register word_t r3 asm("r3") = arg3;
        register word_t r4 asm("r4") = arg4;
        asm volatile(
            "svc $0"
            : "+r"(r0)
            : "r"(r1), "r"(r2), "r"(r3), "r"(r4)
            : "memory"
        );
        return r0;
    }

    static word_t call5(Operation op, word_t arg1, word_t arg2, word_t arg3,
                        word_t arg4, word_t arg5) {
        register word_t r0 asm("r0") = op;
        register word_t r1 asm("r1") = arg1;
        register word_t r2 asm("r2") = arg2;
        register word_t r3 asm("r3") = arg3;
        register word_t r4 asm("r4") = arg4;
        register word_t r5 asm("r5") = arg5;
        asm volatile(
            "svc $0"
            : "+r"(r0)
            : "r"(r1), "r"(r2), "r"(r3), "r"(r4), "r"(r5)
            : "memory"
        );
        return r0;
    }
};

}
