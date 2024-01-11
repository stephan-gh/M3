#
# Copyright (C) 2016-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
# Economic rights: Technische Universitaet Dresden (Germany)
#
# Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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
.extern isr_stack
.extern isr_common

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

    ldr     sp, =isr_stack
    mov     fp, #0

    // perform initialization
    bl      init
    mov     sp, r0

    // start idling
    ldr     r0, =sleep
    mov     pc, r0
END_FUNC(_start)

.section .user_text

BEGIN_FUNC(sleep)
_shutdown:
.global _shutdown
    // enable interrupts and user mode
    mov     r0, #0x10
    msr     CPSR, r0

1:  wfi
    b       1b
END_FUNC(sleep)
