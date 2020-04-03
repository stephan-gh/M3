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

#include <assert.h>

#if defined(__x86_64__)
#   include "arch/x86_64/ISR.h"
#elif defined(__arm__)
#   include "arch/arm/ISR.h"
#elif defined(__riscv)
#   include "arch/riscv/ISR.h"
#else
#   error "Unsupported ISA"
#endif

namespace m3 {

class ISR : public ISRBase {
public:
    typedef ExceptionState State;

    typedef void *(*isr_func)(State *state);

    /**
     * Initializes interrupt and exception handling
     */
    static void init(uintptr_t kstack) asm("isr_init");

    /**
     * Registers <func> for vector <idx>
     */
    static void reg(size_t idx, isr_func func) asm("isr_reg");

    /**
     * Enables interrupts
     */
    static void enable_irqs() asm("isr_enable");

    /**
     * Sets the stack pointer for ISRs to <sp>.
     */
    static void set_sp(uintptr_t sp) asm("isr_set_sp");

    /**
     * Will handle an interrupt/exception based on the given state. Calls the registered function
     * for the vector in <state>.
     */
    static void *handler(State *state) asm("irq_handler");

private:
    static void *null_handler(State *state) {
        return state;
    }

    static isr_func isrs[ISR_COUNT];
};

}
