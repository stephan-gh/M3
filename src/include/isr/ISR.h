/*
 * Copyright (C) 2016-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * This file is part of M3 (Microkernel for Minimalist Manycores).
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
#include <base/Exceptions.h>

#include <assert.h>

#if defined(__x86_64__)
#   include "arch/x86_64/ISR.h"
#elif defined(__arm__)
#   include "arch/arm/ISR.h"
#else
#   error "Unsupported ISA"
#endif

namespace m3 {

class ISR : public ISRBase {
public:
    static void init();

    static m3::Exceptions::isr_func *table() {
        return isrs;
    }

    static void reg(size_t idx, Exceptions::isr_func func) {
        isrs[idx] = func;
    }

private:
    static void *handler(m3::Exceptions::State *state) asm("irq_handler");

    static void *null_handler(m3::Exceptions::State *state) {
        return state;
    }

    static Exceptions::isr_func isrs[ISR_COUNT];
};

}
