/**
 * Copyright (C) 2016, René Küttner <rene.kuettner@.tu-dresden.de>
 * Economic rights: Technische Universität Dresden (Germany)
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

#include <base/DTU.h>
#include <base/Env.h>

#include <isr/ISR.h>

namespace m3 {

EXTERN_C void *isr_stack;
EXTERN_C void *exc_common;

ISR::isr_func ISR::isrs[ISR_COUNT];

void *ISR::handler(State *state) {
    size_t vec = state->cause & 0xF;
    if(state->cause & 0x80000000)
        vec = 16 + (state->cause & 0xF);
    // don't repeat the ECALL instruction
    if(vec >= 8 && vec <= 10)
        state->sepc += 4;
    return isrs[vec](state);
}

void ISR::enable_irqs() {
    asm volatile (
        // delegate all interrupts and exceptions to supervisor mode
        "li     a1, 0x333\n"
        "csrw   mideleg, a1\n"
        "li     a0, -1\n"
        "csrw   medeleg, a0\n"

        // set stack pointer for exceptions
        "csrw    sscratch, %0\n"

        // enable interrupts
        "csrr   a0, sie\n"
        "or     a0, a0, a1\n"
        "csrw   sie, a0\n"

        // return to supervisor mode
        "csrr    a0, mstatus\n"
        "li      a1, 1 << 11\n"
        "or      a0, a0, a1      # MPP = S\n"
        "or      a0, a0, 1 << 1  # SIE = 1\n"
        "csrw    mstatus, a0\n"

        // jump to 1:
        "la      a0, 1f\n"
        "csrw    mepc, a0\n"

        // set vector address
        "slli    a0, %1, 2       # shift addr left; leave mode as direct\n"
        "csrw    stvec, a0       # STVEC = exc_common\n"

        // we need a fence to ensure that the previous CSR accesses are recognized by mret
        "fence.i\n"

        // go!
        "mret\n"

        "1:\n"
        : : "r"(&isr_stack), "r"(&exc_common) : "a0", "a1"
    );
}

void ISR::init() {
    for(size_t i = 0; i < ISR_COUNT; ++i)
        reg(i, null_handler);
}

}
