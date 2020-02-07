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

#include <base/Asm.h>

# void thread_switch(m3::Thread::Regs *old, m3::Thread::Regs *new)
BEGIN_FUNC(thread_switch)
    stm     r0, {r0,r4-r11,r13,r14}
    mrs     r2, CPSR
    str     r2, [r0, #11 * 4]

    ldr     r2, [r1, #11 * 4]
    msr     CPSR_f, r2
    ldm     r1, {r0,r4-r11,r13,r14}

    mov     pc, lr
END_FUNC(thread_switch)
