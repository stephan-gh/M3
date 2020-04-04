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
        return TCU::get().reply(rg.ep()->id(), reply, size, msg);
    }

    static Errors::Code call(SendGate &sg, const void *msg, size_t size,
                             RecvGate &rg, const TCU::Message **reply) noexcept {
        Errors::Code res = send(sg, msg, size, 0, rg);
        if(res != Errors::NONE)
            return res;
        return receive(rg, &sg, reply);
    }

    static const TCU::Message *fetch_msg(RecvGate &rg) noexcept {
        return TCU::get().fetch_msg(rg.ep()->id());
    }

    static void ack_msg(RecvGate &rg, const TCU::Message *msg) noexcept {
        TCU::get().ack_msg(rg.ep()->id(), msg);
    }

    static Errors::Code receive(RecvGate &rg, SendGate *sg, const TCU::Message **reply) noexcept {
        while(1) {
            *reply = TCU::get().fetch_msg(rg.ep()->id());
            if(*reply)
                return Errors::NONE;

            if(sg && EXPECT_FALSE(!TCU::get().is_valid(sg->ep()->id())))
                return Errors::EP_INVALID;

            sleep();
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

    static void drop_msgs(epid_t ep, label_t label) noexcept {
        TCU::get().drop_msgs(ep, label);
    }

    static void sleep() noexcept {
        sleep_for(0);
    }
    static void sleep_with_tcu(uint64_t cycles) noexcept {
        TCU::get().sleep_for(cycles);
    }
    static void sleep_for(uint64_t cycles) noexcept {
        // TODO PEMux does not support sleeps with timeout atm
        if(env()->shared && cycles == 0)
            PEXCalls::call1(Operation::SLEEP, cycles);
        else
            sleep_with_tcu(cycles);
    }
};

}
