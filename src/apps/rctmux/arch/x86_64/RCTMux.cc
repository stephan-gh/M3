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
#include <base/Exceptions.h>
#include <base/KIF.h>
#include <base/RCTMux.h>

#include <isr/ISR.h>

#include "../../RCTMux.h"
#include "VMA.h"

EXTERN_C void *idle_stack;
EXTERN_C void idle();

namespace RCTMux {
namespace Arch {

void init() {
    m3::ISR::init();
    m3::ISR::reg(14, VMA::mmu_pf);
    m3::ISR::reg(64, VMA::dtu_irq);
}

void *init_state(m3::Exceptions::State *state) {
    m3::Env *senv = m3::env();
    senv->isrs = reinterpret_cast<uintptr_t>(m3::ISR::table());

    // init State
    state->rax = 0xDEADBEEF;    // tell crt0 that we've set the SP
    state->rbx = 0;
    state->rcx = 0;
    state->rdx = 0;
    state->rsi = 0;
    state->rdi = 0;
    state->r8  = 0;
    state->r9  = 0;
    state->r10 = 0;
    state->r11 = 0;
    state->r12 = 0;
    state->r13 = 0;
    state->r14 = 0;
    state->r15 = 0;

    state->cs  = (m3::ISR::SEG_UCODE << 3) | 3;
    state->ss  = (m3::ISR::SEG_UDATA << 3) | 3;
    state->rip = senv->entry;
    state->rsp = senv->sp;
    state->rbp = 0;
    state->rflags = 0x200;  // enable interrupts

    return state;
}

void stop_state(m3::Exceptions::State *state) {
    state->rip = reinterpret_cast<uintptr_t>(&idle);
    state->rsp = reinterpret_cast<uintptr_t>(&idle_stack);
    state->rbp = state->rsp;
    state->rflags = 0x200;  // enable interrupts
}

}
}
