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

#ifndef PACKED
#   define PACKED      __attribute__((packed))
#endif
#ifndef UNREACHED
#   define UNREACHED   __builtin_unreachable()
#endif

typedef unsigned char uint8_t;
typedef unsigned short uint16_t;
typedef unsigned int uint32_t;
typedef unsigned long long uint64_t;

#if defined(__arm__)
typedef unsigned int size_t;
typedef unsigned int uintptr_t;
#else
typedef unsigned long size_t;
typedef unsigned long uintptr_t;
#endif
typedef unsigned long epid_t;
typedef unsigned long peid_t;
typedef unsigned vpeid_t;
typedef unsigned long word_t;
typedef word_t label_t;
typedef uint16_t crd_t;
typedef uint64_t reg_t;
typedef uint64_t goff_t;

inline void compiler_barrier() {
    asm volatile ("" : : : "memory");
}

#if defined(__arm__)
inline void memory_barrier() {
    asm volatile ("dmb" : : : "memory");
}

inline uint64_t read8b(uintptr_t addr) {
    uint64_t res;
    asm volatile (
        "ldrd %0, [%1]"
        : "=r"(res)
        : "r"(addr)
    );
    return res;
}

inline void write8b(uintptr_t addr, uint64_t val) {
    asm volatile (
        "strd %0, [%1]"
        : : "r"(val), "r"(addr)
    );
}
#else
inline void memory_barrier() {
    asm volatile ("mfence" : : : "memory");
}

inline uint64_t read8b(uintptr_t addr) {
    uint64_t res;
    asm volatile (
        "mov (%1), %0"
        : "=r"(res)
        : "r"(addr)
    );
    return res;
}

inline void write8b(uintptr_t addr, uint64_t val) {
    asm volatile (
        "mov %0, (%1)"
        :
        : "r"(val), "r"(addr)
    );
}
#endif

enum Error {
    NONE,
    MISS_CREDITS,
    NO_RING_SPACE,
    VPE_GONE,
    PAGEFAULT,
    NO_MAPPING,
    INV_EP,
    ABORT,
    REPLY_DISABLED,
    INV_MSG,
    INV_ARGS,
    NO_PERM,
};

class DTU {
public:
    static const uintptr_t BASE_ADDR        = 0xF0000000;
    static const size_t DTU_REGS            = 6;
    static const size_t CMD_REGS            = 5;
    static const size_t EP_REGS             = 3;

    // actual max is 64k - 1; use less for better alignment
    static const size_t MAX_PKT_SIZE        = 60 * 1024;

    static const vpeid_t INVALID_VPE        = 0xFFFF;

    enum class DtuRegs {
        FEATURES            = 0,
        ROOT_PT             = 1,
        PF_EP               = 2,
        CUR_TIME            = 3,
        CLEAR_IRQ           = 4,
        CLOCK               = 5,
    };

    enum class CmdRegs {
        COMMAND             = DTU_REGS + 0,
        ABORT               = DTU_REGS + 1,
        DATA                = DTU_REGS + 2,
        OFFSET              = DTU_REGS + 3,
        REPLY_LABEL         = DTU_REGS + 4,
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
            FL_GRANT_CREDITS    = 1 << 1,
            FL_REPLY_ENABLED    = 1 << 2,
            FL_PAGEFAULT        = 1 << 3,
        };

        uint8_t flags; // if bit 0 is set its a reply, if bit 1 is set we grant credits
        uint8_t senderPe;
        uint8_t senderEp;
        uint8_t replyEp;   // for a normal message this is the reply epId
                           // for a reply this is the enpoint that receives credits
        uint16_t length;
        uint16_t replySize;

        uint64_t replylabel;
        uint64_t label;
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
                        (static_cast<reg_t>(reply_eps) << 25) |
                        (static_cast<reg_t>(bufSize) << 33) |
                        (static_cast<reg_t>(msgSize) << 39));
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
        write_reg(ep, 1, (static_cast<reg_t>(pe & 0xFF) << 8) |
                         (static_cast<reg_t>(dstep & 0xFF) << 0));
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
        write_reg(CmdRegs::DATA, reinterpret_cast<reg_t>(msg) | (static_cast<reg_t>(size) << 48));
        if(replylbl)
            write_reg(CmdRegs::REPLY_LABEL, replylbl);
        compiler_barrier();
        write_reg(CmdRegs::COMMAND, build_command(ep, CmdOpCode::SEND, 0, reply_ep));

        return get_error();
    }

    static Error reply(epid_t ep, const void *reply, size_t size, const Message *msg) {
        write_reg(CmdRegs::DATA, reinterpret_cast<reg_t>(reply) | (static_cast<reg_t>(size) << 48));
        compiler_barrier();
        write_reg(CmdRegs::COMMAND, build_command(ep, CmdOpCode::REPLY, 0, reinterpret_cast<reg_t>(msg)));

        return get_error();
    }

    static Error transfer(reg_t cmd, uintptr_t data, size_t size, goff_t off) {
        size_t left = size;
        while(left > 0) {
            size_t amount = left < MAX_PKT_SIZE ? left : MAX_PKT_SIZE;
            write_reg(CmdRegs::DATA, data | (static_cast<reg_t>(amount) << 48));
            compiler_barrier();
            write_reg(CmdRegs::COMMAND, cmd | (static_cast<reg_t>(off) << 17));

            left -= amount;
            data += amount;
            off += amount;

            Error res = get_error();
            if(res != Error::NONE)
                return res;
        }
        return Error::NONE;
    }

    static Error read(epid_t ep, void *data, size_t size, goff_t off, unsigned flags) {
        uintptr_t dataaddr = reinterpret_cast<uintptr_t>(data);
        reg_t cmd = build_command(ep, CmdOpCode::READ, flags, 0);
        Error res = transfer(cmd, dataaddr, size, off);
        memory_barrier();
        return res;
    }

    static Error write(epid_t ep, const void *data, size_t size, goff_t off, unsigned flags) {
        uintptr_t dataaddr = reinterpret_cast<uintptr_t>(data);
        reg_t cmd = build_command(ep, CmdOpCode::WRITE, flags, 0);
        return transfer(cmd, dataaddr, size, off);
    }

    static const Message *fetch_msg(epid_t ep) {
        write_reg(CmdRegs::COMMAND, build_command(ep, CmdOpCode::FETCH_MSG));
        memory_barrier();
        return reinterpret_cast<const Message*>(read_reg(CmdRegs::OFFSET));
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
                return static_cast<Error>((cmd >> 13) & 0xF);
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
                (static_cast<reg_t>(flags) << 12 |
                arg << 17);
    }
};
