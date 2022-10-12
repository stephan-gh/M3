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

#define SLOG(lvl, msg, ...) LOG(ServiceLog, lvl, msg, __VA_ARGS__)

namespace m3 {

class ServiceLog {
    ServiceLog() = delete;

public:
    enum Level {
        DEF = 1 << 0,
        KEYB = 1 << 1,
        HASH = 1 << 2,
        LOADGEN = 1 << 3,
        NIC = 1 << 4,
        NET = 1 << 5,
        NET_ALL = 1 << 6,
        TIMER = 1 << 7,
    };

    static const int level = DEF;
};

}
