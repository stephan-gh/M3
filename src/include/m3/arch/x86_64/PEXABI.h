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
#include <base/PEXIF.h>

namespace m3 {

class PEXABI {
public:
    static word_t call1(Operation op, word_t arg1) {
        return call2(op, arg1, 0);
    }

    static word_t call2(Operation op, UNUSED word_t arg1, UNUSED word_t arg2) {
        word_t res = op;
#if defined(__gem5__)
        asm volatile(
            "int $63"
            : "+a"(res)
            : "c"(arg1), "d"(arg2)
            : "memory"
        );
#endif
        return res;
    }
};

}
