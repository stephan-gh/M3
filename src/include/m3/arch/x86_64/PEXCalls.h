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
        word_t res = op;
        asm volatile(
            "int $63"
            : "+a"(res)
            : "c"(arg1)
            : "memory"
        );
        return res;
    }

    static word_t call2(Operation op, word_t arg1, word_t arg2) {
        word_t res = op;
        asm volatile(
            "int $63"
            : "+a"(res)
            : "c"(arg1), "d"(arg2)
            : "memory"
        );
        return res;
    }

    static word_t call4(Operation op, word_t arg1, word_t arg2, word_t arg3, word_t arg4) {
        word_t res = op;
        asm volatile(
            "int $63"
            : "+a"(res)
            : "c"(arg1), "d"(arg2), "D"(arg3), "S"(arg4)
            : "memory"
        );
        return res;
    }

    static word_t call5(Operation op, word_t arg1, word_t arg2, word_t arg3,
                        word_t arg4, word_t arg5) {
        register word_t r8 __asm__ ("r8") = arg5;
        word_t res = op;
        asm volatile(
            "int $63"
            : "+a"(res)
            : "c"(arg1), "d"(arg2), "D"(arg3), "S"(arg4), "r"(r8)
            : "memory"
        );
        return res;
    }
};

}
