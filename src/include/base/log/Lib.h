/*
 * Copyright (C) 2016-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#pragma once

#include <base/log/Log.h>

#define LLOG(lvl, msg)  LOG(LibLog, lvl, msg)

namespace m3 {

class LibLog {
    LibLog() = delete;

public:
    enum Level {
        DTU         = 1 << 0,
        DTUERR      = 1 << 1,
        IPC         = 1 << 2,
        TRACE       = 1 << 3,
        IRQS        = 1 << 4,
        SHM         = 1 << 5,
        HEAP        = 1 << 6,
        FS          = 1 << 7,
        SERV        = 1 << 8,
        THREAD      = 1 << 9,
        ACCEL       = 1 << 10,
        FILES       = 1 << 11,
        NET 		= 1 << 12,
    };

    static const int level = DTUERR;
};

}
