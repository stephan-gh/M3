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

#define LOG(flag, fmt, ...)                                                    \
    do {                                                                       \
        if(EXPECT_FALSE(should_log(flag))) {                                   \
            m3::detail::format_rec<0, 0>(fmt, m3::Serial::get(), __VA_ARGS__); \
            m3::Serial::get().write('\n');                                     \
        }                                                                      \
    }                                                                          \
    while(0)

namespace m3 {

// Note: needs to be in sync with Rust's LogFlags
enum LogFlags {
    Info = 1 << 0,
    Debug = 1 << 1,
    Error = 1 << 2,

    LibFS = 1 << 3,
    LibServ = 1 << 4,
    LibNet = 1 << 5,
    LibXlate = 1 << 6,
    LibThread = 1 << 7,
    LibSQueue = 1 << 8,
    LibDirPipe = 1 << 9,
};

struct Log {
    Log();

    uint64_t flags;

    static Log inst;
};

static inline bool should_log(uint64_t flag) {
#if defined(bench)
    return flag == LogFlags::Info || flag == LogFlags::Error;
#else
    return (Log::inst.flags & flag) != 0;
#endif
}

}
