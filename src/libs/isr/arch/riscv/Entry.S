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

BEGIN_FUNC(isr_enable)
    // delegate all interrupts and exceptions to supervisor mode
    li     a1, 0x333
    csrw   mideleg, a1
    li     a0, -1
    csrw   medeleg, a0

    // set stack pointer for exceptions
    la      a0, isr_stack
    csrw    sscratch, a0

    // enable interrupts
    csrr   a0, sie
    or     a0, a0, a1
    csrw   sie, a0

    // return to supervisor mode
    csrr    a0, mstatus
    li      a1, 1 << 11
    or      a0, a0, a1      # MPP = S
    or      a0, a0, 1 << 1  # SIE = 1
    csrw    mstatus, a0

    // jump to 1:
    la      a0, 1f
    csrw    mepc, a0

    // set vector address
    la      a0, exc_common
    slli    a0, a0, 2       # shift addr left; leave mode as direct
    csrw    stvec, a0       # STVEC = exc_common

    // we need a fence to ensure that the previous CSR accesses are recognized by mret
    fence.i

    // go!
    mret
1:
    ret
END_FUNC(isr_enable)

BEGIN_FUNC(exc_common)
    # swap x31 with SSCRATCH to have a usable register
    csrrw   x31, sscratch, x31

    # calculate base address
    addi    x31, x31, -(8*33)

    # save GPRs
    sd      x1, 8*0(x31)
    sd      x2, 8*1(x31)
    sd      x3, 8*2(x31)
    sd      x4, 8*3(x31)
    sd      x5, 8*4(x31)
    sd      x6, 8*5(x31)
    sd      x7, 8*6(x31)
    sd      x8, 8*7(x31)
    sd      x9, 8*8(x31)
    sd      x10, 8*9(x31)
    sd      x11, 8*10(x31)
    sd      x12, 8*11(x31)
    sd      x13, 8*12(x31)
    sd      x14, 8*13(x31)
    sd      x15, 8*14(x31)
    sd      x16, 8*15(x31)
    sd      x17, 8*16(x31)
    sd      x18, 8*17(x31)
    sd      x19, 8*18(x31)
    sd      x20, 8*19(x31)
    sd      x21, 8*20(x31)
    sd      x22, 8*21(x31)
    sd      x23, 8*22(x31)
    sd      x24, 8*23(x31)
    sd      x25, 8*24(x31)
    sd      x26, 8*25(x31)
    sd      x27, 8*26(x31)
    sd      x28, 8*27(x31)
    sd      x29, 8*28(x31)
    sd      x30, 8*29(x31)

    # swap SSCRATCH with GPR and save it
    mv      sp, x31
    addi    x31, x31, 8*33
    csrrw   x31, sscratch, x31
    sd      x31, 8*30(sp)
    # save SCAUSE
    csrr    s1, scause
    sd      s1, 8*31(sp)
    # save SEPC
    csrr    s1, sepc
    sd      s1, 8*32(sp)

    mv      a0, sp
    jal     irq_handler
    mv      x1, sp

    # restore SEPC
    ld      s1, 8*32(x1)
    csrw    sepc, s1

    # restore GPRs
    ld      x31, 8*30(x1)
    ld      x30, 8*29(x1)
    ld      x29, 8*28(x1)
    ld      x28, 8*27(x1)
    ld      x27, 8*26(x1)
    ld      x26, 8*25(x1)
    ld      x25, 8*24(x1)
    ld      x24, 8*23(x1)
    ld      x23, 8*22(x1)
    ld      x22, 8*21(x1)
    ld      x21, 8*20(x1)
    ld      x20, 8*19(x1)
    ld      x19, 8*18(x1)
    ld      x18, 8*17(x1)
    ld      x17, 8*16(x1)
    ld      x16, 8*15(x1)
    ld      x15, 8*14(x1)
    ld      x14, 8*13(x1)
    ld      x13, 8*12(x1)
    ld      x12, 8*11(x1)
    ld      x11, 8*10(x1)
    ld      x10, 8*9(x1)
    ld      x9, 8*8(x1)
    ld      x8, 8*7(x1)
    ld      x7, 8*6(x1)
    ld      x6, 8*5(x1)
    ld      x5, 8*4(x1)
    ld      x4, 8*3(x1)
    ld      x3, 8*2(x1)
    ld      x2, 8*1(x1)
    ld      x1, 8*0(x1)

    sret
END_FUNC(exc_common)

.bss

    .align 16
    .space 16384, 0
isr_stack:
