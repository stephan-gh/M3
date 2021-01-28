/*
 * Copyright (C) 2016, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/TCU.h>
#include <base/Env.h>

#include <m3/PEXCalls.h>
#include <m3/com/MemGate.h>
#include <m3/com/RecvGate.h>
#include <m3/com/SendGate.h>

namespace m3 {

class TCUIf {
public:
    static void sleep() noexcept {
        sleep_for(0);
    }

    static void sleep_for(uint64_t nanos) noexcept {
        if(env()->shared || nanos != 0)
            PEXCalls::call2(Operation::SLEEP, nanos, TCU::INVALID_EP);
        else
            TCU::get().wait_for_msg(TCU::INVALID_EP);
    }

    static void wait_for_msg(epid_t ep) noexcept {
        if(env()->shared)
            PEXCalls::call2(Operation::SLEEP, 0, ep);
        else
            TCU::get().wait_for_msg(ep);
    }

    static void flush_invalidate() noexcept {
        PEXCalls::call2(Operation::FLUSH_INV, 0, 0);
    }
};

}
