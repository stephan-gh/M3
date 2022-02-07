#
# Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
# Economic rights: Technische Universitaet Dresden (Germany)
#
# Copyright (C) 2019-2020 Nils Asmussen, Barkhausen Institut
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
#include <base/Config.h>

.weak baremetal_stack
.extern env_run

BEGIN_FUNC(_start)
    # stack pointer already set?
    mov     $0xDEADBEEF, %rcx
    cmp     %rcx, %rax
    je      1f

    # set default stack pointer
    mov     $baremetal_stack, %rsp
    mov     $0, %rbp

1:
    call    env_run

    # just to be sure
    hlt
END_FUNC(_start)
