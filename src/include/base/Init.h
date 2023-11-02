/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

#pragma once

#include <base/Compiler.h>

#define INIT_PRIO_LXDEV     INIT_PRIO(101)
#define INIT_PRIO_LXWAIT    INIT_PRIO(102)

#define INIT_PRIO_LOG       INIT_PRIO(103)
#define INIT_PRIO_SENDQUEUE INIT_PRIO(104)
#define INIT_PRIO_TCU       INIT_PRIO(105)

#define INIT_PRIO_RECVBUF   INIT_PRIO(106)
#define INIT_PRIO_RECVGATE  INIT_PRIO(107)
#define INIT_PRIO_SYSCALLS  INIT_PRIO(108)

#define INIT_PRIO_ACT       INIT_PRIO(109)
#define INIT_PRIO_VFS       INIT_PRIO(110)
#define INIT_PRIO_STREAM    INIT_PRIO(111)

#define INIT_PRIO_LXENV     INIT_PRIO(112)
