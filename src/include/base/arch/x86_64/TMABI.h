/*
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

#pragma once

#include <base/Common.h>
#include <base/Errors.h>
#include <base/TMIF.h>

namespace m3 {

class TMABI {
public:
    static Errors::Code call1(Operation op, word_t arg1) {
        return call2(op, arg1, 0);
    }

    static Errors::Code call2(Operation op, UNUSED word_t arg1, UNUSED word_t arg2) {
        word_t res = op;
        asm volatile("int $63" : "+a"(res) : "c"(arg1), "d"(arg2) : "memory");
        return static_cast<Errors::Code>(res);
    }

    static Errors::Code call3(Operation op, UNUSED word_t arg1, UNUSED word_t arg2,
                              UNUSED word_t arg3) {
        word_t res = op;
        asm volatile("int $63" : "+a"(res) : "c"(arg1), "d"(arg2), "D"(arg3) : "memory");
        return static_cast<Errors::Code>(res);
    }

    static Errors::Code call4(Operation op, UNUSED word_t arg1, UNUSED word_t arg2,
                              UNUSED word_t arg3, UNUSED word_t arg4) {
        word_t res = op;
        asm volatile("int $63" : "+a"(res) : "c"(arg1), "d"(arg2), "D"(arg3), "S"(arg4) : "memory");
        return static_cast<Errors::Code>(res);
    }
};

}
