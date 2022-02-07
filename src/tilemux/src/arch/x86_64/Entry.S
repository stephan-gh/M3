#
# Copyright (C) 2016-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
# Economic rights: Technische Universitaet Dresden (Germany)
#
# Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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
.extern isr_stack

BEGIN_FUNC(_start)
    xor     %rax, %rax
    mov     $isr_stack, %rsp
    mov     $0, %rbp
    call    init

    // return to idle code
    pushq   $(4 << 3) | 3   // ss
    pushq   $isr_stack      // rsp
    pushq   $0x200          // rflags: interrupts enabled
    pushq   $(3 << 3) | 3   // cs
    pushq   $sleep          // rip
    iretq
END_FUNC(_start)

.section .user_text

BEGIN_FUNC(sleep)
1:  hlt
    jmp     1b
END_FUNC(sleep)
