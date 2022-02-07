/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2020 Nils Asmussen, Barkhausen Institut
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

#include <base/stream/Serial.h>
#include <base/Env.h>

// on the host, we need a lock here because of a race between the TCU-thread and CPU-thread
#if defined(__host__)
#   define LOCK()     m3::env()->log_lock()
#   define UNLOCK()   m3::env()->log_unlock()
#else
#   define LOCK()
#   define UNLOCK()
#endif

#define LOG(cls, lvl, expr)                                 \
    do {                                                    \
        if(m3::cls::level & (m3::cls::lvl)) {               \
            LOCK();                                         \
            m3::Serial::get() << expr << '\n';              \
            UNLOCK();                                       \
        }                                                   \
    }                                                       \
    while(0)
