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
    static Errors::Code send(SendGate &sg, const void *msg, size_t size,
                             label_t replylbl, RecvGate &rg) noexcept {
        const EP &sep = sg.activate();
        epid_t rep = rg.ep() ? rg.ep()->id() : TCU::NO_REPLIES;
        return TCU::get().send(sep.id(), msg, size, replylbl, rep);
    }

    static Errors::Code reply(RecvGate &rg, const void *reply, size_t size,
                              const TCU::Message *msg) noexcept {
        size_t msg_off = TCU::msg_to_offset(rg.address(), msg);
        return TCU::get().reply(rg.ep()->id(), reply, size, msg_off);
    }

    static Errors::Code call(SendGate &sg, const void *msg, size_t size,
                             RecvGate &rg, const TCU::Message **reply) noexcept {
        Errors::Code res = send(sg, msg, size, 0, rg);
        if(res != Errors::NONE)
            return res;
        return receive(rg, &sg, reply);
    }

    static const TCU::Message *fetch_msg(RecvGate &rg) noexcept {
        size_t msg_off = TCU::get().fetch_msg(rg.ep()->id());
        if(msg_off != static_cast<size_t>(-1))
            return TCU::offset_to_msg(rg.address(), msg_off);
        return nullptr;
    }

    static void ack_msg(RecvGate &rg, const TCU::Message *msg) noexcept {
        size_t msg_off = TCU::msg_to_offset(rg.address(), msg);
        TCU::get().ack_msg(rg.ep()->id(), msg_off);
    }

    static Errors::Code receive(RecvGate &rg, SendGate *sg, const TCU::Message **reply) noexcept {
        // if the PE is shared with someone else that wants to run, poll a couple of times to
        // prevent too frequent/unnecessary switches.
        int polling = env()->shared ? 200 : 1;
        while(1) {
            for(int i = 0; i < polling; ++i) {
                *reply = fetch_msg(rg);
                if(*reply)
                    return Errors::NONE;
            }

            if(sg && EXPECT_FALSE(!TCU::get().is_valid(sg->ep()->id())))
                return Errors::EP_INVALID;

            wait_for_msg(rg.ep()->id());
        }
        UNREACHED;
    }

    static Errors::Code read(MemGate &mg, void *data, size_t size, goff_t off, uint flags) noexcept {
        const EP &ep = mg.activate();
        return TCU::get().read(ep.id(), data, size, off, flags);
    }

    static Errors::Code write(MemGate &mg, const void *data, size_t size,
                              goff_t off, uint flags) noexcept {
        const EP &ep = mg.activate();
        return TCU::get().write(ep.id(), data, size, off, flags);
    }

    static void drop_msgs(RecvGate &rg, label_t label) noexcept {
        TCU::get().drop_msgs(rg.address(), rg.ep()->id(), label);
    }

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
};

}
