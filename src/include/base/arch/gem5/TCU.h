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

#include <base/Common.h>
#include <base/util/Util.h>
#include <base/CPU.h>
#include <base/Env.h>
#include <base/Errors.h>
#include <assert.h>

namespace kernel {
class TCU;
class TCURegs;
class TCUState;
class ISR;
class SendQueue;
class SyscallHandler;
class VPE;
class PEMux;
class WorkLoop;
}

namespace m3 {

class TCUIf;

class TCU {
    friend class kernel::TCU;
    friend class kernel::TCURegs;
    friend class kernel::TCUState;
    friend class kernel::ISR;
    friend class kernel::SendQueue;
    friend class kernel::SyscallHandler;
    friend class kernel::VPE;
    friend class kernel::PEMux;
    friend class kernel::WorkLoop;
    friend class TCUIf;

    explicit TCU() {
    }

public:
    typedef uint64_t reg_t;

    static const uintptr_t MMIO_ADDR        = 0xF0000000;
    static const size_t MMIO_SIZE           = PAGE_SIZE * 2;
    static const uintptr_t MMIO_PRIV_ADDR   = MMIO_ADDR + MMIO_SIZE;
    static const size_t MMIO_PRIV_SIZE      = PAGE_SIZE;

    static const reg_t NO_REPLIES           = 0xFFFF;

private:
    static const size_t TCU_REGS            = 4;
    static const size_t PRIV_REGS           = 6;
    static const size_t CMD_REGS            = 4;
    static const size_t EP_REGS             = 3;

    // actual max is 64k - 1; use less for better alignment
    static const size_t MAX_PKT_SIZE        = 60 * 1024;

    enum class TCURegs {
        FEATURES            = 0,
        CUR_TIME            = 1,
        CLEAR_IRQ           = 2,
        CLOCK               = 3,
    };

    enum class PrivRegs {
        CORE_REQ            = 0,
        CORE_RESP           = 1,
        PRIV_CMD            = 2,
        EXT_CMD             = 3,
        CUR_VPE             = 4,
        OLD_VPE             = 5,
    };

    enum class CmdRegs {
        COMMAND             = TCU_REGS + 0,
        ABORT               = TCU_REGS + 1,
        DATA                = TCU_REGS + 2,
        ARG1                = TCU_REGS + 3,
    };

    enum StatusFlags : reg_t {
        PRIV                = 1 << 0,
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

    enum class PrivCmdOpCode {
        IDLE                = 0,
        INV_PAGE            = 1,
        INV_TLB             = 2,
        XCHG_VPE            = 3,
    };

    enum class ExtCmdOpCode {
        IDLE                = 0,
        INV_EP              = 1,
        INV_REPLY           = 2,
        RESET               = 3,
    };

public:
    enum class EventType {
        MSG_RECV,
        CRD_RECV,
        EP_INVAL,
    };

    enum EventMask : reg_t {
        MSG_RECV    = 1 << static_cast<reg_t>(EventType::MSG_RECV),
        CRD_RECV    = 1 << static_cast<reg_t>(EventType::CRD_RECV),
        EP_INVAL    = 1 << static_cast<reg_t>(EventType::EP_INVAL),
    };

    enum MemFlags : reg_t {
        R                   = 1 << 0,
        W                   = 1 << 1,
    };

    enum CmdFlags {
        NOPF                = 1,
    };

    struct Header {
        enum {
            FL_REPLY            = 1 << 0,
            FL_PAGEFAULT        = 1 << 1,
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

    static const epid_t KPEX_SEP            = 0;
    static const epid_t KPEX_REP            = 1;
    static const epid_t PEXUP_REP           = 2;
    static const epid_t PEXUP_RPLEP         = 3;

    static const epid_t SYSC_SEP_OFF        = 0;
    static const epid_t SYSC_REP_OFF        = 1;
    static const epid_t UPCALL_REP_OFF      = 2;
    static const epid_t UPCALL_RPLEP_OFF    = 3;
    static const epid_t DEF_REP_OFF         = 4;
    static const epid_t PG_SEP_OFF          = 5;
    static const epid_t PG_REP_OFF          = 6;

    static const epid_t FIRST_USER_EP       = 4;
    static const epid_t STD_EPS_COUNT       = 7;

    static TCU &get() {
        return inst;
    }

    static peid_t gaddr_to_pe(gaddr_t noc) {
        return (noc >> 56) - 0x80;
    }
    static gaddr_t gaddr_to_virt(gaddr_t noc) {
        return noc & ((static_cast<gaddr_t>(1) << 56) - 1);
    }
    static gaddr_t build_gaddr(peid_t pe, gaddr_t virt) {
        return (static_cast<gaddr_t>(0x80 + pe) << 56) | virt;
    }

    bool has_missing_credits(epid_t ep) const {
        reg_t r0 = read_reg(ep, 0);
        uint16_t cur = (r0 >> 19) & 0x3F;
        uint16_t max = (r0 >> 25) & 0x3F;
        return cur < max;
    }

    bool has_credits(epid_t ep) const {
        reg_t r0 = read_reg(ep, 0);
        uint16_t cur = (r0 >> 19) & 0x3F;
        return cur > 0;
    }

    bool is_valid(epid_t ep) const {
        reg_t r0 = read_reg(ep, 0);
        return static_cast<EpType>(r0 & 0x7) != EpType::INVALID;
    }

    cycles_t tsc() const {
        return read_reg(TCURegs::CUR_TIME);
    }
    cycles_t clock() const {
        return read_reg(TCURegs::CLOCK);
    }

    void print(const char *str, size_t len);

private:
    Errors::Code send(epid_t ep, const void *msg, size_t size, label_t replylbl, epid_t reply_ep);
    Errors::Code reply(epid_t ep, const void *reply, size_t size, const Message *msg);
    Errors::Code read(epid_t ep, void *msg, size_t size, goff_t off, uint flags);
    Errors::Code write(epid_t ep, const void *msg, size_t size, goff_t off, uint flags);

    const Message *fetch_msg(epid_t ep) const {
        write_reg(CmdRegs::COMMAND, build_command(ep, CmdOpCode::FETCH_MSG));
        CPU::memory_barrier();
        return reinterpret_cast<const Message*>(read_reg(CmdRegs::ARG1));
    }

    reg_t fetch_events() const {
        write_reg(CmdRegs::COMMAND, build_command(0, CmdOpCode::FETCH_EVENTS));
        CPU::memory_barrier();
        return read_reg(CmdRegs::ARG1);
    }

    void ack_msg(epid_t ep, const Message *msg) {
        // ensure that we are really done with the message before acking it
        CPU::memory_barrier();
        reg_t off = reinterpret_cast<reg_t>(msg);
        write_reg(CmdRegs::COMMAND, build_command(ep, CmdOpCode::ACK_MSG, 0, off));
        // ensure that we don't do something else before the ack
        CPU::memory_barrier();
    }

    void sleep() {
        sleep_for(0);
    }
    void sleep_for(uint64_t cycles) {
        wait_for_msg(0xFFFF, cycles);
    }
    void wait_for_msg(epid_t ep, uint64_t timeout = 0) {
        write_reg(CmdRegs::ARG1, (static_cast<reg_t>(ep) << 48) | timeout);
        CPU::compiler_barrier();
        write_reg(CmdRegs::COMMAND, build_command(0, CmdOpCode::SLEEP));
        get_error();
    }

    void drop_msgs(epid_t ep, label_t label) {
        // we assume that the one that used the label can no longer send messages. thus, if there
        // are no messages yet, we are done.
        word_t unread = read_reg(ep, 2) >> 32;
        if(unread == 0)
            return;

        reg_t r0 = read_reg(ep, 0);
        goff_t base = read_reg(ep, 1);
        size_t bufsize = static_cast<size_t>(1) << ((r0 >> 35) & 0x3F);
        size_t msgsize = (r0 >> 41) & 0x3F;
        for(size_t i = 0; i < bufsize; ++i) {
            if(unread & (static_cast<size_t>(1) << i)) {
                m3::TCU::Message *msg = reinterpret_cast<m3::TCU::Message*>(base + (i << msgsize));
                if(msg->label == label)
                    ack_msg(ep, msg);
            }
        }
    }

    reg_t get_core_req() const {
        return read_reg(PrivRegs::CORE_REQ);
    }
    void set_core_req(reg_t val) {
        write_reg(PrivRegs::CORE_REQ, val);
    }
    void set_core_resp(reg_t val) {
        write_reg(PrivRegs::CORE_RESP, val);
    }

    void clear_irq() {
        write_reg(TCURegs::CLEAR_IRQ, 1);
    }

    static Errors::Code get_error() {
        while(true) {
            reg_t cmd = read_reg(CmdRegs::COMMAND);
            if(static_cast<CmdOpCode>(cmd & 0xF) == CmdOpCode::IDLE)
                return static_cast<Errors::Code>((cmd >> 21) & 0xF);
        }
        UNREACHED;
    }

    static reg_t read_reg(TCURegs reg) {
        return read_reg(static_cast<size_t>(reg));
    }
    static reg_t read_reg(PrivRegs reg) {
        return read_reg(((PAGE_SIZE * 2) / sizeof(reg_t)) + static_cast<size_t>(reg));
    }
    static reg_t read_reg(CmdRegs reg) {
        return read_reg(static_cast<size_t>(reg));
    }
    static reg_t read_reg(epid_t ep, size_t idx) {
        return read_reg(TCU_REGS + CMD_REGS + EP_REGS * ep + idx);
    }
    static reg_t read_reg(size_t idx) {
        return CPU::read8b(MMIO_ADDR + idx * sizeof(reg_t));
    }

    static void write_reg(TCURegs reg, reg_t value) {
        write_reg(static_cast<size_t>(reg), value);
    }
    static void write_reg(PrivRegs reg, reg_t value) {
        write_reg(((PAGE_SIZE * 2) / sizeof(reg_t)) + static_cast<size_t>(reg), value);
    }
    static void write_reg(CmdRegs reg, reg_t value) {
        write_reg(static_cast<size_t>(reg), value);
    }
    static void write_reg(size_t idx, reg_t value) {
        CPU::write8b(MMIO_ADDR + idx * sizeof(reg_t), value);
    }

    static uintptr_t tcu_reg_addr(TCURegs reg) {
        return MMIO_ADDR + static_cast<size_t>(reg) * sizeof(reg_t);
    }
    static uintptr_t priv_reg_addr(PrivRegs reg) {
        return MMIO_ADDR + (PAGE_SIZE * 2) + static_cast<size_t>(reg) * sizeof(reg_t);
    }
    static uintptr_t cmd_reg_addr(CmdRegs reg) {
        return MMIO_ADDR + static_cast<size_t>(reg) * sizeof(reg_t);
    }
    static uintptr_t ep_regs_addr(epid_t ep) {
        return MMIO_ADDR + (TCU_REGS + CMD_REGS + ep * EP_REGS) * sizeof(reg_t);
    }
    static uintptr_t buffer_addr() {
        size_t regCount = TCU_REGS + CMD_REGS + EP_COUNT * EP_REGS;
        return MMIO_ADDR + regCount * sizeof(reg_t);
    }

    static reg_t build_command(epid_t ep, CmdOpCode c, uint flags = 0, reg_t arg = 0) {
        return static_cast<reg_t>(c) |
                (static_cast<reg_t>(ep) << 4) |
                (static_cast<reg_t>(flags) << 20 |
                arg << 25);
    }

    static TCU inst;
};

}
