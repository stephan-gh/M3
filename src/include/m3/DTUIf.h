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

#if defined(__gem5__)
static const bool USE_PEXCALLS = true;
#else
static const bool USE_PEXCALLS = false;
#endif

namespace m3 {

class DTUIf {
    static Errors::Code get_error(word_t res) {
        long err = static_cast<long>(res);
        if(err < 0)
            return static_cast<Errors::Code>(-err);
        return Errors::NONE;
    }

public:
    static Errors::Code send(epid_t ep, const void *msg, size_t size, label_t replylbl, epid_t reply_ep) {
        if(USE_PEXCALLS) {
            return get_error(PEXCalls::call5(Operation::SEND,
                                             ep,
                                             reinterpret_cast<word_t>(msg),
                                             size,
                                             replylbl,
                                             reply_ep));
        }
        else
            return DTU::get().send(ep, msg, size, replylbl, reply_ep);
    }

    static Errors::Code reply(epid_t ep, const void *reply, size_t size, const DTU::Message *msg) {
        if(USE_PEXCALLS) {
            return get_error(PEXCalls::call4(Operation::REPLY,
                                             ep,
                                             reinterpret_cast<word_t>(reply),
                                             size,
                                             reinterpret_cast<word_t>(msg)));
        }
        else
            return DTU::get().reply(ep, reply, size, msg);
    }

    static Errors::Code call(epid_t ep, const void *msg, size_t size,
                             epid_t reply_ep, const DTU::Message **reply) {
        if(USE_PEXCALLS) {
            word_t res = PEXCalls::call4(Operation::CALL,
                                         ep,
                                         reinterpret_cast<word_t>(msg),
                                         size,
                                         reply_ep);
            Errors::Code err = get_error(res);
            if(err == Errors::NONE)
                *reply = reinterpret_cast<const DTU::Message*>(res);
            return err;
        }
        else {
            Errors::Code res = send(ep, msg, size, 0, reply_ep);
            if(res != Errors::NONE)
                return res;
            return receive(reply_ep, ep, reply);
        }
    }

    static const DTU::Message *fetch_msg(epid_t ep) {
        if(USE_PEXCALLS) {
            word_t res = PEXCalls::call1(Operation::FETCH, ep);
            Errors::Code err = get_error(res);
            if(err != Errors::NONE)
                return nullptr;
            return reinterpret_cast<const DTU::Message*>(res);
        }
        else
            return DTU::get().fetch_msg(ep);
    }

    static void mark_read(epid_t ep, const DTU::Message *msg) {
        if(USE_PEXCALLS)
            PEXCalls::call2(Operation::ACK, ep, reinterpret_cast<word_t>(msg));
        else
            DTU::get().mark_read(ep, msg);
    }

    static Errors::Code receive(epid_t rep, epid_t sep, const DTU::Message **reply) {
        if(USE_PEXCALLS) {
            word_t res = PEXCalls::call2(Operation::RECV, rep, sep);
            Errors::Code err = get_error(res);
            if(err == Errors::NONE)
                *reply = reinterpret_cast<const DTU::Message*>(res);
            return err;
        }
        else {
            while(1) {
                *reply = DTU::get().fetch_msg(rep);
                if(*reply)
                    return Errors::NONE;

                // fetch the events first
                DTU::get().fetch_events();
                // now check whether the endpoint is still valid. if the EP has been invalidated before
                // the line above, we'll notice that with this check. if the EP is invalidated between
                // the line above and the sleep command, the DTU will refuse to suspend the core.
                if(sep != EP_COUNT && EXPECT_FALSE(!DTU::get().is_valid(sep)))
                    return Errors::EP_INVALID;

                DTU::get().sleep();
            }
            UNREACHED;
        }
    }

    static Errors::Code read(epid_t ep, void *data, size_t size, goff_t off, uint flags) {
        if(USE_PEXCALLS) {
            return get_error(PEXCalls::call5(Operation::READ,
                                             ep,
                                             reinterpret_cast<word_t>(data),
                                             size,
                                             off,
                                             flags));
        }
        else
            return DTU::get().read(ep, data, size, off, flags);
    }
    static Errors::Code write(epid_t ep, const void *data, size_t size, goff_t off, uint flags) {
        if(USE_PEXCALLS) {
            return get_error(PEXCalls::call5(Operation::WRITE,
                                             ep,
                                             reinterpret_cast<word_t>(data),
                                             size,
                                             off,
                                             flags));
        }
        else
            return DTU::get().write(ep, data, size, off, flags);
    }

    static void drop_msgs(epid_t ep, label_t label) {
        DTU::get().drop_msgs(ep, label);
    }

    static void sleep() {
        sleep_for(0);
    }
    static void sleep_for(uint64_t cycles) {
        if(USE_PEXCALLS)
            PEXCalls::call1(Operation::SLEEP, cycles);
        else
            DTU::get().sleep_for(cycles);
    }
};

}
