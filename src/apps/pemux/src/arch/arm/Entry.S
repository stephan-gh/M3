#
# Copyright (C) 2013, Nils Asmussen <nils@os.inf.tu-dresden.de>
# Economic rights: Technische Universitaet Dresden (Germany)
#
# This file is part of M3 (Microkernel for Minimalist Manycores).
#
# M3 is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License version 2 as
# published by the Free Software Foundation.
#
# M3 is distributed in the hope that it will be useful, but
# WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
# General Public License version 2 for more details.
#

#include <base/Asm.h>

.extern init
.extern sleep
.extern isr_stack
.global idle_stack
.global idle

.text

BEGIN_FUNC(_start)
    mrc     p15, 0, r0, c10, c2, 0  // PRRR
    orr     r0, r0, #0x00000002     // set tr0 = 0x2 (normal memory)
    mcr     p15, 0, r0, c10, c2, 0

    mrc     p15, 0, r0, c10, c2, 1  // NMRR
    orr     r0, r0, #0x00000001     // set ir0 = 0x1 (write-back, write-allocate)
    orr     r0, r0, #0x00010000     // set or0 = 0x1 (write-back, write-allocate)
    mcr     p15, 0, r0, c10, c2, 1

    mrc     p15, 0, r0, c1, c0, 0   // SCTLR
    orr     r0, r0, #0x00001000     // enable I-cache
    orr     r0, r0, #0x00000004     // enable D-cache
    orr     r0, r0, #0x10000000     // enable TEX remap
    mcr     p15, 0, r0, c1, c0, 0

    // setup stack
    ldr     sp, =isr_stack
    mov     fp, #0

    // perform initialization, if necessary
    bl      init

    // load user sp
    ldr     r0, =idle_stack
    push    {r0}
    ldmia   sp, {r13}^
    // enable interrupts and user mode
    mov     r0, #0x10
    msr     CPSR, r0
    // start idling
    ldr     r0, =idle
    mov     pc, r0
END_FUNC(_start)

BEGIN_FUNC(idle)
    // TODO: the isb (or something else) is required, because of a bug in the O3 CPU as its seems.
    // The problem is that the restore of lr within a rfe is performed *after* the setting of
    // lr by the bl instruction below. Consequently, lr is overwritten and we return to somewhere
    // else with the next "bx lr".
    isb
1:  bl      sleep
    b       1b
END_FUNC(idle)

.bss

    .space 4096
idle_stack:
