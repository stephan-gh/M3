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

    static const reg_t INVALID_VPE = 0xFFFF;

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
        reg_t bufSize = static_cast<reg_t>(order - msgorder);
        reg_t msgSize = static_cast<reg_t>(msgorder);
        write_reg(ep, 0, static_cast<reg_t>(m3::TCU::EpType::RECEIVE) |
                        (static_cast<reg_t>(INVALID_VPE) << 3) |
                        (static_cast<reg_t>(reply_eps) << 19) |
                        (static_cast<reg_t>(bufSize) << 35) |
                        (static_cast<reg_t>(msgSize) << 41));
        write_reg(ep, 1, buf);
        write_reg(ep, 2, static_cast<reg_t>(unread) << 32 | occupied);
    }

    static void config_send(epid_t ep, label_t lbl, peid_t pe, epid_t dstep,
                            unsigned msgorder, unsigned credits) {
        write_reg(ep, 0, static_cast<reg_t>(m3::TCU::EpType::SEND) |
                        (static_cast<reg_t>(INVALID_VPE) << 3) |
                        (static_cast<reg_t>(credits) << 19) |
                        (static_cast<reg_t>(credits) << 25) |
                        (static_cast<reg_t>(msgorder) << 31));
        write_reg(ep, 1, (static_cast<reg_t>(pe) << 16) |
                         (static_cast<reg_t>(dstep) << 0));
        write_reg(ep, 2, lbl);
    }

    static void config_mem(epid_t ep, peid_t pe, goff_t addr, size_t size, int perm) {
        write_reg(ep, 0, static_cast<reg_t>(m3::TCU::EpType::MEMORY) |
                        (static_cast<reg_t>(INVALID_VPE) << 3) |
                        (static_cast<reg_t>(perm) << 19) |
                        (static_cast<reg_t>(pe) << 23));
        write_reg(ep, 1, addr);
        write_reg(ep, 2, size);
    }

    static void write_reg(epid_t ep, size_t idx, reg_t value) {
        size_t off = m3::TCU::EXT_REGS + m3::TCU::UNPRIV_REGS + m3::TCU::EP_REGS * ep + idx;
        m3::TCU::write_reg(off, value);
    }
};

}
