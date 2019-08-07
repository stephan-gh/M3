/**
 * Copyright (C) 2015, René Küttner <rene.kuettner@.tu-dresden.de>
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

#include <base/CPU.h>
#include <base/DTU.h>
#include <base/Env.h>
#include <base/Exceptions.h>
#include <base/RCTMux.h>

#include "RCTMux.h"
#include "Print.h"

EXTERN_C void *isr_stack;
EXTERN_C void _start();

namespace RCTMux {

static void *restore();
static void signal();

__attribute__((section(".rctmux"))) static volatile uint64_t rctmux_flags[2];

static inline uint64_t flags_get() {
    return rctmux_flags[1];
}

static inline void flags_set(uint64_t flags) {
    rctmux_flags[1] = flags;
}

void init() {
    Arch::init();
}

void sleep() {
    m3::DTU::get().sleep();
}

void *ctxsw_protocol(void *s) {
    uint64_t flags = flags_get();

    if(flags & m3::RESTORE) {
        s = restore();
        return s;
    }

    if(flags & m3::WAITING)
        signal();

    return s;
}

static void *restore() {
    uint64_t flags = flags_get();

    // notify the kernel as early as possible
    signal();

    m3::Env *senv = m3::env();
    // remember the current PE (might have changed since last switch)
    senv->pe = flags >> 32;

    auto *stacktop = reinterpret_cast<m3::Exceptions::State*>(&isr_stack) - 1;
    // if we get here, there is an application to jump to

    // remember exit location
    senv->exitaddr = reinterpret_cast<uintptr_t>(&_start);

    // initialize the state to be able to resume from it
    return Arch::init_state(stacktop);
}

static void signal() {
    m3::CPU::memory_barrier();
    // tell the kernel that we are ready
    flags_set(m3::SIGNAL);
}

}
