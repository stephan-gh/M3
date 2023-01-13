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

    static Errors::Code call2(Operation op, word_t arg1, word_t arg2) {
        register word_t r0 asm("r0") = op;
        register word_t r1 asm("r1") = arg1;
        register word_t r2 asm("r2") = arg2;
        asm volatile("svc $0" : "+r"(r0) : "r"(r1), "r"(r2) : "memory");
        return static_cast<Errors::Code>(r0);
    }

    static Errors::Code call3(Operation op, word_t arg1, word_t arg2, word_t arg3) {
        register word_t r0 asm("r0") = op;
        register word_t r1 asm("r1") = arg1;
        register word_t r2 asm("r2") = arg2;
        register word_t r3 asm("r3") = arg3;
        asm volatile("svc $0" : "+r"(r0) : "r"(r1), "r"(r2), "r"(r3) : "memory");
        return static_cast<Errors::Code>(r0);
    }

    static Errors::Code call4(Operation op, word_t arg1, word_t arg2, word_t arg3, word_t arg4) {
        register word_t r0 asm("r0") = op;
        register word_t r1 asm("r1") = arg1;
        register word_t r2 asm("r2") = arg2;
        register word_t r3 asm("r3") = arg3;
        register word_t r4 asm("r4") = arg4;
        asm volatile("svc $0" : "+r"(r0) : "r"(r1), "r"(r2), "r"(r3), "r"(r4) : "memory");
        return static_cast<Errors::Code>(r0);
    }
};

}
