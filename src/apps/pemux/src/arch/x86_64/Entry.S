#
# Copyright (C) 2015, Nils Asmussen <nils@os.inf.tu-dresden.de>
# Economic rights: Technische Universitaet Dresden (Germany)
#
# This file is part of M3 (Microkernel-based SysteM for Heterogeneous Manycores).
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

BEGIN_FUNC(_start)
    xor     %rax, %rax
    // use idle stack during init and sleep to not interfere with ISRs
    mov     $idle_stack, %rsp
    mov     $0, %rbp
    call    init

    // return to idle code
    pushq   $(1 << 3) | 0   // ss
    pushq   $idle_stack     // rsp
    pushq   $0x200          // rflags: interrupts enabled
    pushq   $(2 << 3) | 0   // cs
    pushq   $sleep          // rip
    iretq
END_FUNC(_start)

.bss

    .space 16384
idle_stack:
