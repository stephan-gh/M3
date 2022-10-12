/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2021 Nils Asmussen, Barkhausen Institut
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

#include <base/log/Log.h>

#define LLOG(lvl, fmt, ...) LOG(LibLog, lvl, fmt, __VA_ARGS__)

namespace m3 {

class LibLog {
    LibLog() = delete;

public:
    enum Level {
        DEF = 1 << 0,
        TCU = 1 << 1,
        TCU_SLEEP = 1 << 2,
        TCUERR = 1 << 3,
        IPC = 1 << 4,
        TRACE = 1 << 5,
        IRQS = 1 << 6,
        SHM = 1 << 7,
        HEAP = 1 << 8,
        FS = 1 << 9,
        SERV = 1 << 10,
        THREAD = 1 << 11,
        ACCEL = 1 << 12,
        FILES = 1 << 13,
        NET = 1 << 14,
        DIRPIPE = 1 << 15,
    };

    static const int level = DEF | TCUERR;
};

}
