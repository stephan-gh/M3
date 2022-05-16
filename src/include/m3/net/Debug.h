/*
 * Copyright (C) 2022 Nils Asmussen, Barkhausen Institut
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

#include <base/CPU.h>

namespace m3 {

enum NetLogEvent {
    SubmitData = 1,
    SentPacket,
    RecvPacket,
    FetchData,
    RecvConnected,
    RecvClosed,
    RecvRemoteClosed,
    StartedWaiting,
    StoppedWaiting,
};

template<typename T1, typename T2>
static inline void log_net(UNUSED NetLogEvent event, UNUSED T1 arg1, UNUSED T2 arg2) {
#if defined(__gem5__)
    uint64_t msg = static_cast<uint64_t>(event) | (static_cast<uint64_t>(arg1) << 8) |
                   (static_cast<uint64_t>(arg2) << 16);
    CPU::gem5_debug(msg);
#endif
}

}
