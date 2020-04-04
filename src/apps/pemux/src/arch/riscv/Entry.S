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
.extern isr_stack

BEGIN_FUNC(_start)
    la      sp, isr_stack
    jal     init

    // return to user mode
    li      a0, 1 << 8
    csrc    sstatus, a0
    // enable interrupts
    li      a0, 1 << 5
    csrs    sstatus, a0

    // jump to sleep function
    la      a0, sleep
    csrw    sepc, a0

    // go!
    sret
END_FUNC(_start)

.section .user_text

BEGIN_FUNC(sleep)
1:  // write 0xFFFF to ARG_1
    li      a0, 0xF0000038
    li      a1, 0xFFFF000000000000
    sd      a1, 0(a0)
    // write SLEEP to COMMAND
    li      a0, 0xF0000020
    li      a1, 0x7
    sd      a1, 0(a0)
    j       1b
END_FUNC(sleep)
