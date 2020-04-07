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

#include <base/TCU.h>
#include <base/Env.h>

#include <isr/ISR.h>

extern "C" void isr_setup(uintptr_t kstack);

namespace m3 {

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

void ISR::init(uintptr_t kstack) {
    for(size_t i = 0; i < ISR_COUNT; ++i)
        reg(i, null_handler);
    isr_setup(kstack);
}

void ISR::enable_irqs() {
    // set SIE to 1
    asm volatile ("csrs sstatus, %0" : : "r"(1 << 1));
}

}
