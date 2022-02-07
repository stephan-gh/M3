#
# Copyright (C) 2016-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
# Economic rights: Technische Universitaet Dresden (Germany)
#
# Copyright (C) 2019-2020 Nils Asmussen, Barkhausen Institut
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

.extern isr_handler
.extern isr_stack

.text

.macro BUILD_ENTRY, no
BEGIN_FUNC(isr_\no)
    // save spsr and r14 onto the supervisor stack
    srsdb   #0x13!

    // switch to supervisor mode, interrupts disabled
    cpsid   i,#0x13

    // save all regs
    push    {r0-r12, lr}

    // remember isr number
    mov     r0, #\no
    push    {r0}

    b       isr_common
END_FUNC(isr_\no)
.endm

BUILD_ENTRY 0
BUILD_ENTRY 1
BUILD_ENTRY 2
BUILD_ENTRY 3
BUILD_ENTRY 4
BUILD_ENTRY 6
BUILD_ENTRY 7

BEGIN_FUNC(isr_common)
    // save user space sp/lr
    sub     sp, #8
    stmia   sp, {r13, r14}^

    // argument for isr_handler (saved state)
    mov     r0, sp

    // start with a new stack
    ldr     sp, =isr_stack

    // call handler
    bl      isr_handler

    // restore state from state given by isr_handler
    mov     sp, r0

    // restore user space sp/lr
    ldmia   sp, {r13, r14}^
    add     sp, #12

    // restore all regs
    pop     {r0-r12, r14}

    // return to user
    rfe     sp!
END_FUNC(isr_common)
