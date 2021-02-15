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

#define SLOG(lvl, msg)  LOG(ServiceLog, lvl, msg)

namespace m3 {

class ServiceLog {
    ServiceLog() = delete;

public:
    enum Level {
        KEYB        = 1 << 0,
        HASH        = 1 << 1,
        LOADGEN     = 1 << 2,
        NIC         = 1 << 3,
        NET         = 1 << 4,
        NET_ALL     = 1 << 5,
        TIMER       = 1 << 6,
    };

    static const int level = 0;
};

}
