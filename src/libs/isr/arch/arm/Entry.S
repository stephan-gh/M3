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

.extern irq_handler

.global isr_stack

.text

.macro BUILD_ENTRY, no
BEGIN_FUNC(entry_\no)
    // save spsr and r14 onto the supervisor stack
    srsdb   #0x13!

    // switch to supervisor mode, interrupts disabled
    cpsid   i,#0x13

    // save all regs
    push    {r0-r12, lr}

    // remember entry
    mov     r0, #\no
    push    {r0}

    b       exc_common
END_FUNC(entry_\no)
.endm

BUILD_ENTRY 0
BUILD_ENTRY 1
BUILD_ENTRY 2
BUILD_ENTRY 3
BUILD_ENTRY 4
BUILD_ENTRY 6
BUILD_ENTRY 7

BEGIN_FUNC(exc_common)
    // save user space sp/lr
    sub     sp, #8
    stmia   sp, {r13, r14}^

    // make sure the stack is aligned
    mov     r0, sp
    tst     sp, #4
    subeq   sp, #4
    push    {r0}

    // call handler (sp already in r0)
    bl      irq_handler
    mov     sp, r0

    // restore user space sp/lr
    ldmia   sp, {r13, r14}^
    add     sp, #12

    // restore all regs
    pop     {r0-r12, r14}

    // return to user
    // TODO does that work with the O3 CPU?
    rfe     sp!
END_FUNC(exc_common)

.bss

    .align      2
    .space 16384,0
isr_stack:
