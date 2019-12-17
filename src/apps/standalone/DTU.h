/*
 * Copyright (C) 2016-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include "common.h"

enum Error {
    NONE,
    MISS_CREDITS,
    NO_RING_SPACE,
    PAGEFAULT,
    INV_EP,
    ABORT,
    INV_MSG,
    INV_ARGS,
    NO_PERM,
};

class DTU {
public:
    static const uintptr_t BASE_ADDR        = 0xF0000000;
    static const size_t DTU_REGS            = 4;
    static const size_t CMD_REGS            = 4;
    static const size_t EP_REGS             = 3;

    static const vpeid_t INVALID_VPE        = 0xFFFF;

    static const reg_t NO_REPLIES           = 0xFFFF;

    enum class DtuRegs {
        FEATURES            = 0,
        CUR_TIME            = 1,
        CLEAR_IRQ           = 2,
        CLOCK               = 3,
    };

    enum class CmdRegs {
        COMMAND             = DTU_REGS + 0,
        ABORT               = DTU_REGS + 1,
        DATA                = DTU_REGS + 2,
        ARG1                = DTU_REGS + 3,
    };

    enum MemFlags : reg_t {
        R                   = 1 << 0,
        W                   = 1 << 1,
        RW                  = R | W,
    };

    enum class EpType {
        INVALID,
        SEND,
        RECEIVE,
        MEMORY
    };

    enum class CmdOpCode {
        IDLE                = 0,
        SEND                = 1,
        REPLY               = 2,
        READ                = 3,
        WRITE               = 4,
        FETCH_MSG           = 5,
        FETCH_EVENTS        = 6,
        SET_EVENT           = 7,
        ACK_MSG             = 8,
        SLEEP               = 9,
        PRINT               = 10,
    };

    enum {
        ABORT_VPE           = 1,
        ABORT_CMD           = 2,
    };

    struct alignas(8) Header {
        enum {
            FL_REPLY            = 1 << 0,
        };

        uint8_t flags : 2,
                replySize : 6;
        uint8_t senderPe;
        uint16_t senderEp;
        uint16_t replyEp;   // for a normal message this is the reply epId
                            // for a reply this is the enpoint that receives credits
        uint16_t length;

        uint32_t replylabel;
        uint32_t label;
    } PACKED;

    struct Message : Header {
        epid_t send_ep() const {
            return senderEp;
        }
        epid_t reply_ep() const {
            return replyEp;
        }

        unsigned char data[];
    } PACKED;

    static bool is_valid(epid_t ep) {
        reg_t r0 = read_reg(ep, 0);
        return static_cast<EpType>(r0 & 0x7) != EpType::INVALID;
    }

    static void config_recv(epid_t ep, goff_t buf, unsigned order,
                            unsigned msgorder, unsigned reply_eps) {
        reg_t bufSize = static_cast<reg_t>(order - msgorder);
        reg_t msgSize = static_cast<reg_t>(msgorder);
        write_reg(ep, 0, static_cast<reg_t>(EpType::RECEIVE) |
                        (static_cast<reg_t>(INVALID_VPE) << 3) |
                        (static_cast<reg_t>(reply_eps) << 19) |
                        (static_cast<reg_t>(bufSize) << 35) |
                        (static_cast<reg_t>(msgSize) << 41));
        write_reg(ep, 1, buf);
        write_reg(ep, 2, 0);
    }

    static void config_send(epid_t ep, label_t lbl, peid_t pe, epid_t dstep,
                            unsigned msgorder, unsigned credits) {
        write_reg(ep, 0, static_cast<reg_t>(EpType::SEND) |
                        (static_cast<reg_t>(INVALID_VPE) << 3) |
                        (static_cast<reg_t>(credits) << 19) |
                        (static_cast<reg_t>(credits) << 25) |
                        (static_cast<reg_t>(msgorder) << 31));
        write_reg(ep, 1, (static_cast<reg_t>(pe) << 16) |
                         (static_cast<reg_t>(dstep) << 0));
        write_reg(ep, 2, lbl);
    }

    static void config_mem(epid_t ep, peid_t pe, goff_t addr, size_t size, int perm) {
        write_reg(ep, 0, static_cast<reg_t>(EpType::MEMORY) |
                        (static_cast<reg_t>(INVALID_VPE) << 3) |
                        (static_cast<reg_t>(perm) << 19) |
                        (static_cast<reg_t>(pe) << 23));
        write_reg(ep, 1, addr);
        write_reg(ep, 2, size);
    }

    static Error send(epid_t ep, const void *msg, size_t size, label_t replylbl, epid_t reply_ep) {
        write_reg(CmdRegs::DATA, reinterpret_cast<reg_t>(msg) | (static_cast<reg_t>(size) << 32));
        if(replylbl)
            write_reg(CmdRegs::ARG1, replylbl);
        compiler_barrier();
        write_reg(CmdRegs::COMMAND, build_command(ep, CmdOpCode::SEND, 0, reply_ep));

        return get_error();
    }

    static Error reply(epid_t ep, const void *reply, size_t size, const Message *msg) {
        write_reg(CmdRegs::DATA, reinterpret_cast<reg_t>(reply) | (static_cast<reg_t>(size) << 32));
        compiler_barrier();
        write_reg(CmdRegs::COMMAND, build_command(ep, CmdOpCode::REPLY, 0, reinterpret_cast<reg_t>(msg)));

        return get_error();
    }

    static Error read(epid_t ep, void *data, size_t size, goff_t off, unsigned flags) {
        write_reg(CmdRegs::DATA, reinterpret_cast<reg_t>(data) | (static_cast<reg_t>(size) << 32));
        write_reg(CmdRegs::ARG1, off);
        compiler_barrier();
        write_reg(CmdRegs::COMMAND, build_command(ep, CmdOpCode::READ, flags));
        Error res = get_error();
        memory_barrier();
        return res;
    }

    static Error write(epid_t ep, const void *data, size_t size, goff_t off, unsigned flags) {
        write_reg(CmdRegs::DATA, reinterpret_cast<reg_t>(data) | (static_cast<reg_t>(size) << 32));
        write_reg(CmdRegs::ARG1, off);
        compiler_barrier();
        write_reg(CmdRegs::COMMAND, build_command(ep, CmdOpCode::WRITE, flags));
        return get_error();
    }

    static const Message *fetch_msg(epid_t ep) {
        write_reg(CmdRegs::COMMAND, build_command(ep, CmdOpCode::FETCH_MSG));
        memory_barrier();
        return reinterpret_cast<const Message*>(read_reg(CmdRegs::ARG1));
    }

    static void mark_read(epid_t ep, const Message *msg) {
        // ensure that we are really done with the message before acking it
        memory_barrier();
        reg_t off = reinterpret_cast<reg_t>(msg);
        write_reg(CmdRegs::COMMAND, build_command(ep, CmdOpCode::ACK_MSG, 0, off));
        // ensure that we don't do something else before the ack
        get_error();
    }

    static Error get_error() {
        while(true) {
            reg_t cmd = read_reg(CmdRegs::COMMAND);
            if(static_cast<CmdOpCode>(cmd & 0xF) == CmdOpCode::IDLE)
                return static_cast<Error>((cmd >> 21) & 0xF);
        }
        UNREACHED;
    }

    static reg_t read_reg(DtuRegs reg) {
        return read_reg(static_cast<size_t>(reg));
    }
    static reg_t read_reg(CmdRegs reg) {
        return read_reg(static_cast<size_t>(reg));
    }
    static reg_t read_reg(epid_t ep, size_t idx) {
        return read_reg(DTU_REGS + CMD_REGS + EP_REGS * ep + idx);
    }
    static reg_t read_reg(size_t idx) {
        return read8b(BASE_ADDR + idx * sizeof(reg_t));
    }

    static void write_reg(DtuRegs reg, reg_t value) {
        write_reg(static_cast<size_t>(reg), value);
    }
    static void write_reg(CmdRegs reg, reg_t value) {
        write_reg(static_cast<size_t>(reg), value);
    }
    static void write_reg(epid_t ep, size_t idx, reg_t value) {
        write_reg(DTU_REGS + CMD_REGS + EP_REGS * ep + idx, value);
    }
    static void write_reg(size_t idx, reg_t value) {
        write8b(BASE_ADDR + idx * sizeof(reg_t), value);
    }

    static reg_t build_command(epid_t ep, CmdOpCode c, unsigned flags = 0, reg_t arg = 0) {
        return static_cast<reg_t>(c) |
                (static_cast<reg_t>(ep) << 4) |
                (static_cast<reg_t>(flags) << 20 |
                arg << 25);
    }
};
