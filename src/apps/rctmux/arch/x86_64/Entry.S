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

.extern isr_stack
.extern init
.extern sleep

.text

// put that at the beginning so that we know the address
BEGIN_FUNC(_start)
    // we need nops here to ensure that we start executing at the first real instruction. because
    // the DTU simply sets the PC and it might be that the CPU finishes the last instruction
    // afterwards, which increases the PC. so, use more nops than the longest instruction
.rept 32
    nop
.endr

    // disable interrupts in case they are enabled
    cli

    // use a small stack
    mov     $isr_stack, %rsp
    mov     $0, %rbp

    call    init

    // return to idle code
    pushq   $(4 << 3) | 3   // ss
    pushq   $idle_stack     // rsp
    pushq   $0x200          // rflags: interrupts enabled
    pushq   $(3 << 3) | 3   // cs
    pushq   $idle           // rip
    iretq
END_FUNC(_start)

BEGIN_FUNC(idle)
1:  call    sleep
    jmp     1b
END_FUNC(idle)

BEGIN_FUNC(gem5_writefile)
    .long   0x004F040F
    ret
END_FUNC(gem5_writefile)

.bss

    .space 128, 0
idle_stack:
