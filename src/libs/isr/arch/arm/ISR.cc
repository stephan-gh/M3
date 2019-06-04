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

Exceptions::isr_func ISR::isrs[ISR_COUNT];

void *ISR::handler(m3::Exceptions::State *state) {
    // repeat last instruction, except for SWIs
    if(state->vector != 2)
        state->pc -= 4;
    return isrs[state->vector](state);
}

void ISR::enable_irqs() {
    // not yet supported
}

void ISR::init() {
    for(size_t i = 0; i < ISR_COUNT; ++i)
        reg(i, null_handler);
}

}
