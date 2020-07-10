/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/Common.h>
#include <base/TCU.h>

namespace kernel {

class TCU {
public:
    typedef m3::TCU::reg_t reg_t;

    static int credits(epid_t ep) {
        reg_t r0 = m3::TCU::read_reg(ep, 0);
        return (r0 >> 19) & 0x3F;
    }

    static const m3::TCU::Message *fetch_msg(epid_t ep, uintptr_t base) {
        size_t off = m3::TCU::get().fetch_msg(ep);
        if(off == static_cast<size_t>(-1))
            return nullptr;
        return reinterpret_cast<const m3::TCU::Message*>(base + off);
    }

    static m3::Errors::Code ack_msg(epid_t ep, uintptr_t base, const m3::TCU::Message *msg) {
        reg_t off = reinterpret_cast<reg_t>(msg) - base;
        return m3::TCU::get().ack_msg(ep, off);
    }

    static m3::Errors::Code send(epid_t ep, const void *msg, size_t size, label_t replylbl, epid_t reply_ep) {
        return m3::TCU::get().send(ep, msg, size, replylbl, reply_ep);
    }

    static m3::Errors::Code reply(epid_t ep, const void *reply, size_t size, uintptr_t base, const m3::TCU::Message *msg) {
        reg_t off = reinterpret_cast<reg_t>(msg) - base;
        return m3::TCU::get().reply(ep, reply, size, off);
    }

    static m3::Errors::Code read(epid_t ep, void *data, size_t size, goff_t off) {
        return m3::TCU::get().read(ep, data, size, off);
    }

    static m3::Errors::Code write(epid_t ep, const void *data, size_t size, goff_t off) {
        return m3::TCU::get().write(ep, data, size, off);
    }

    static void config_recv(epid_t ep, goff_t buf, unsigned order,
                            unsigned msgorder, unsigned reply_eps,
                            uint32_t occupied = 0, uint32_t unread = 0) {
        m3::TCU::config_recv(ep, buf, order, msgorder, reply_eps, occupied, unread);
    }

    static void config_send(epid_t ep, label_t lbl, peid_t pe, epid_t dstep,
                            unsigned msgorder, unsigned credits) {
        m3::TCU::config_send(ep, lbl, pe, dstep, msgorder, credits);
    }

    static void config_mem(epid_t ep, peid_t pe, goff_t addr, size_t size, int perm) {
        m3::TCU::config_mem(ep, pe, addr, size, perm);
    }
};

}
