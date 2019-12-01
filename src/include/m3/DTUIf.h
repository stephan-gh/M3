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

#include <base/DTU.h>
#include <base/Env.h>

#include <m3/PEXCalls.h>
#include <m3/com/MemGate.h>
#include <m3/com/RecvGate.h>
#include <m3/com/SendGate.h>

namespace m3 {

class DTUIf {
public:
    static Errors::Code send(SendGate &sg, const void *msg, size_t size,
                             label_t replylbl, RecvGate &rg) noexcept {
        const EP &sep = sg.activate();
        return DTU::get().send(sep.id(), msg, size, replylbl, rg.ep()->id());
    }

    static Errors::Code reply(RecvGate &rg, const void *reply, size_t size,
                              const DTU::Message *msg) noexcept {
        return DTU::get().reply(rg.ep()->id(), reply, size, msg);
    }

    static Errors::Code call(SendGate &sg, const void *msg, size_t size,
                             RecvGate &rg, const DTU::Message **reply) noexcept {
        Errors::Code res = send(sg, msg, size, 0, rg);
        if(res != Errors::NONE)
            return res;
        return receive(rg, &sg, reply);
    }

    static const DTU::Message *fetch_msg(RecvGate &rg) noexcept {
        return DTU::get().fetch_msg(rg.ep()->id());
    }

    static void mark_read(RecvGate &rg, const DTU::Message *msg) noexcept {
        DTU::get().mark_read(rg.ep()->id(), msg);
    }

    static Errors::Code receive(RecvGate &rg, SendGate *sg, const DTU::Message **reply) noexcept {
        while(1) {
            *reply = DTU::get().fetch_msg(rg.ep()->id());
            if(*reply)
                return Errors::NONE;

            // fetch the events first
            DTU::get().fetch_events();
            // now check whether the endpoint is still valid. if the EP has been invalidated before
            // the line above, we'll notice that with this check. if the EP is invalidated between
            // the line above and the sleep command, the DTU will refuse to suspend the core.
            if(sg && EXPECT_FALSE(!DTU::get().is_valid(sg->ep()->id())))
                return Errors::EP_INVALID;

            DTU::get().wait_for_msg(rg.ep()->id());
        }
        UNREACHED;
    }

    static Errors::Code read(MemGate &mg, void *data, size_t size, goff_t off, uint flags) noexcept {
        const EP &ep = mg.activate();
        return DTU::get().read(ep.id(), data, size, off, flags);
    }

    static Errors::Code write(MemGate &mg, const void *data, size_t size,
                              goff_t off, uint flags) noexcept {
        const EP &ep = mg.activate();
        return DTU::get().write(ep.id(), data, size, off, flags);
    }

    static void drop_msgs(epid_t ep, label_t label) noexcept {
        DTU::get().drop_msgs(ep, label);
    }

    static void sleep() noexcept {
        sleep_for(0);
    }
    static void sleep_for(uint64_t cycles) noexcept {
        if(env()->shared)
            PEXCalls::call1(Operation::SLEEP, cycles);
        else {
            if(DTU::get().fetch_events() == 0)
                DTU::get().sleep_for(cycles);
        }
    }
};

}
