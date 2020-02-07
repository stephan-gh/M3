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
    # save registers
    mov     %rbx,  0(%rdi)
    mov     %rsp,  8(%rdi)
    mov     %rbp, 16(%rdi)
    mov     %r12, 24(%rdi)
    mov     %r13, 32(%rdi)
    mov     %r14, 40(%rdi)
    mov     %r15, 48(%rdi)
    pushfq                              # load eflags
    popq    56(%rdi)                    # store

    # restore registers
    mov     48(%rsi), %r15
    mov     40(%rsi), %r14
    mov     32(%rsi), %r13
    mov     24(%rsi), %r12
    mov     16(%rsi), %rbp
    mov      8(%rsi), %rsp
    mov      0(%rsi), %rbx
    pushq   56(%rsi)
    popfq                               # load eflags
    mov     64(%rsi), %rdi              # load rdi (necessary for startup)

    ret
END_FUNC(thread_switch)
