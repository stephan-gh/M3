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

#include <m3/PEXCalls.h>
#include <m3/com/MemGate.h>
#include <m3/com/RecvGate.h>
#include <m3/com/SendGate.h>

#if defined(__gem5__)
static const bool USE_PEXCALLS = true;
#else
static const bool USE_PEXCALLS = false;
#endif

namespace m3 {

class DTUIf {
    static Errors::Code get_error(word_t res) noexcept {
        long err = static_cast<long>(res);
        if(err < 0)
            return static_cast<Errors::Code>(-err);
        return Errors::NONE;
    }

public:
    static Errors::Code send(SendGate &sg, const void *msg, size_t size,
                             label_t replylbl, RecvGate &rg) noexcept {
        if(USE_PEXCALLS) {
            return get_error(PEXCalls::call5(Operation::SEND,
                                             sg.sel(),
                                             reinterpret_cast<word_t>(msg),
                                             size,
                                             replylbl,
                                             rg.sel()));
        }
        else {
            epid_t sep = sg.acquire_ep();
            return DTU::get().send(sep, msg, size, replylbl, rg.ep());
        }
    }

    static Errors::Code reply(RecvGate &rg, const void *reply, size_t size,
                              const DTU::Message *msg) noexcept {
        if(USE_PEXCALLS) {
            return get_error(PEXCalls::call4(Operation::REPLY,
                                             rg.sel(),
                                             reinterpret_cast<word_t>(reply),
                                             size,
                                             reinterpret_cast<word_t>(msg)));
        }
        else
            return DTU::get().reply(rg.ep(), reply, size, msg);
    }

    static Errors::Code call(SendGate &sg, const void *msg, size_t size,
                             RecvGate &rg, const DTU::Message **reply) noexcept {
        if(USE_PEXCALLS) {
            word_t res = PEXCalls::call4(Operation::CALL,
                                         sg.sel(),
                                         reinterpret_cast<word_t>(msg),
                                         size,
                                         rg.sel());
            Errors::Code err = get_error(res);
            if(err == Errors::NONE)
                *reply = reinterpret_cast<const DTU::Message*>(res);
            return err;
        }
        else {
            Errors::Code res = send(sg, msg, size, 0, rg);
            if(res != Errors::NONE)
                return res;
            return receive(rg, &sg, reply);
        }
    }

    static const DTU::Message *fetch_msg(RecvGate &rg) noexcept {
        if(USE_PEXCALLS) {
            word_t res = PEXCalls::call1(Operation::FETCH, rg.sel());
            Errors::Code err = get_error(res);
            if(err != Errors::NONE)
                return nullptr;
            return reinterpret_cast<const DTU::Message*>(res);
        }
        else
            return DTU::get().fetch_msg(rg.ep());
    }

    static void mark_read(RecvGate &rg, const DTU::Message *msg) noexcept {
        if(USE_PEXCALLS)
            PEXCalls::call2(Operation::ACK, rg.sel(), reinterpret_cast<word_t>(msg));
        else
            DTU::get().mark_read(rg.ep(), msg);
    }

    static Errors::Code receive(RecvGate &rg, SendGate *sg, const DTU::Message **reply) noexcept {
        if(USE_PEXCALLS) {
            word_t res = PEXCalls::call2(Operation::RECV, rg.sel(), sg ? sg->sel() : ObjCap::INVALID);
            Errors::Code err = get_error(res);
            if(err == Errors::NONE)
                *reply = reinterpret_cast<const DTU::Message*>(res);
            return err;
        }
        else {
            while(1) {
                *reply = DTU::get().fetch_msg(rg.ep());
                if(*reply)
                    return Errors::NONE;

                // fetch the events first
                DTU::get().fetch_events();
                // now check whether the endpoint is still valid. if the EP has been invalidated before
                // the line above, we'll notice that with this check. if the EP is invalidated between
                // the line above and the sleep command, the DTU will refuse to suspend the core.
                if(sg && EXPECT_FALSE(!DTU::get().is_valid(sg->ep())))
                    return Errors::EP_INVALID;

                DTU::get().sleep();
            }
            UNREACHED;
        }
    }

    static Errors::Code read(MemGate &mg, void *data, size_t size, goff_t off, uint flags) noexcept {
        if(USE_PEXCALLS) {
            return get_error(PEXCalls::call5(Operation::READ,
                                             mgate_sel(mg),
                                             reinterpret_cast<word_t>(data),
                                             size,
                                             off,
                                             flags));
        }
        else {
            epid_t ep = mg.acquire_ep();
            return DTU::get().read(ep, data, size, off, flags);
        }
    }

    static Errors::Code write(MemGate &mg, const void *data, size_t size,
                              goff_t off, uint flags) noexcept {
        if(USE_PEXCALLS) {
            return get_error(PEXCalls::call5(Operation::WRITE,
                                             mgate_sel(mg),
                                             reinterpret_cast<word_t>(data),
                                             size,
                                             off,
                                             flags));
        }
        else {
            epid_t ep = mg.acquire_ep();
            return DTU::get().write(ep, data, size, off, flags);
        }
    }

    static Errors::Code reserve_ep(epid_t *ep) noexcept {
        assert(USE_PEXCALLS);
        assert(*ep <= EP_COUNT);

        word_t res = PEXCalls::call1(Operation::RES_EP, *ep);
        Errors::Code err = get_error(res);
        if(err != Errors::NONE)
            return err;

        *ep = res;
        return Errors::NONE;
    }

    static void free_ep(epid_t ep) noexcept {
        assert(USE_PEXCALLS);
        assert(ep < EP_COUNT);

        PEXCalls::call1(Operation::FREE_EP, ep);
    }

    static void activate_gate(Gate &gate, epid_t ep, goff_t addr);

    static void remove_gate(Gate &gate, bool invalidate) noexcept {
        if(USE_PEXCALLS)
            PEXCalls::call2(Operation::REMOVE_GATE, gate.sel(), invalidate);
        else
            EPMux::get().remove(&gate, invalidate);
    }

    static void drop_msgs(epid_t ep, label_t label) noexcept {
        DTU::get().drop_msgs(ep, label);
    }

    static void sleep() noexcept {
        sleep_for(0);
    }
    static void sleep_for(uint64_t cycles) noexcept {
        if(USE_PEXCALLS)
            PEXCalls::call1(Operation::SLEEP, cycles);
        else
            DTU::get().sleep_for(cycles);
    }

private:
    static size_t mgate_sel(MemGate &mg) {
        return mg.sel() == ObjCap::INVALID ? (static_cast<size_t>(1) << 31 | mg.ep()) : mg.sel();
    }
};

}
