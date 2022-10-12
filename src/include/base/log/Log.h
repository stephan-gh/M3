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

#include <base/Env.h>
#include <base/stream/Serial.h>

#define LOG(cls, lvl, fmt, ...)                                                \
    do {                                                                       \
        if(m3::cls::level & (m3::cls::lvl)) {                                  \
            m3::detail::format_rec<0, 0>(fmt, m3::Serial::get(), __VA_ARGS__); \
            m3::Serial::get().write('\n');                                     \
        }                                                                      \
    }                                                                          \
    while(0)
