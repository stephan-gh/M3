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
class AddrSpace;
class DTU;
class DTURegs;
class DTUState;
class SendQueue;
class SyscallHandler;
class VPE;
class WorkLoop;
}

namespace m3 {

class DTUIf;

class DTU {
    friend class kernel::AddrSpace;
    friend class kernel::DTU;
    friend class kernel::DTURegs;
    friend class kernel::DTUState;
    friend class kernel::SendQueue;
    friend class kernel::SyscallHandler;
    friend class kernel::VPE;
    friend class kernel::WorkLoop;
    friend class DTUIf;

    explicit DTU() {
    }

public:
    typedef uint64_t reg_t;

private:
    static const uintptr_t BASE_ADDR        = 0xF0000000;
    static const size_t DTU_REGS            = 10;
    static const size_t REQ_REGS            = 3;
    static const size_t CMD_REGS            = 5;
    static const size_t EP_REGS             = 3;
    static const size_t HD_COUNT            = 128;
    static const size_t HD_REGS             = 2;

    static const size_t CREDITS_UNLIM       = 0xFFFF;
    // actual max is 64k - 1; use less for better alignment
    static const size_t MAX_PKT_SIZE        = 60 * 1024;

    enum class DtuRegs {
        FEATURES            = 0,
        ROOT_PT             = 1,
        PF_EP               = 2,
        VPE_ID              = 3,
        CUR_TIME            = 4,
        IDLE_TIME           = 5,
        EVENTS              = 6,
        EXT_CMD             = 7,
        CLEAR_IRQ           = 8,
        CLOCK               = 9,
    };

    enum class ReqRegs {
        EXT_REQ             = 0,
        XLATE_REQ           = 1,
        XLATE_RESP          = 2,
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

    enum StatusFlags : reg_t {
        PRIV                = 1 << 0,
        PAGEFAULTS          = 1 << 1,
        COM_DISABLED        = 1 << 2,
        IRQ_WAKEUP          = 1 << 3,
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
        SEND_BY             = 2,
        REPLY               = 3,
        READ                = 4,
        WRITE               = 5,
        FETCH_MSG           = 6,
        ACK_MSG             = 7,
        ACK_EVENTS          = 8,
        SLEEP               = 9,
        PRINT               = 10,
    };

    enum class ExtCmdOpCode {
        IDLE                = 0,
        WAKEUP_CORE         = 1,
        INV_EP              = 2,
        INV_PAGE            = 3,
        INV_TLB             = 4,
        RESET               = 5,
        ACK_MSG             = 6,
        FLUSH_CACHE         = 7,
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

    typedef uint64_t pte_t;

    enum CmdFlags {
        NOPF                = 1,
    };

    enum {
        PTE_BITS            = 3,
        PTE_SIZE            = 1 << PTE_BITS,
        LEVEL_CNT           = 4,
        LEVEL_BITS          = PAGE_BITS - PTE_BITS,
        LEVEL_MASK          = (1 << LEVEL_BITS) - 1,
        LPAGE_BITS          = PAGE_BITS + LEVEL_BITS,
        LPAGE_SIZE          = 1UL << LPAGE_BITS,
        LPAGE_MASK          = LPAGE_SIZE - 1,
        PTE_REC_IDX         = 0x10,
    };

    enum {
        PTE_R               = 1,
        PTE_W               = 2,
        PTE_X               = 4,
        PTE_I               = 8,
        PTE_LARGE           = 16,
        PTE_UNCACHED        = 32, // unsupported by DTU, but used for MMU
        PTE_RW              = PTE_R | PTE_W,
        PTE_RWX             = PTE_RW | PTE_X,
        PTE_IRWX            = PTE_RWX | PTE_I,
    };

    enum {
        ABORT_VPE           = 1,
        ABORT_CMD           = 2,
    };

    enum ExtReqOpCode {
        INV_PAGE            = 0,
        PEMUX               = 1,
        STOP                = 2,
    };

    struct alignas(8) ReplyHeader {
        enum {
            FL_REPLY            = 1 << 0,
            FL_GRANT_CREDITS    = 1 << 1,
            FL_REPLY_ENABLED    = 1 << 2,
            FL_PAGEFAULT        = 1 << 3,
            FL_REPLY_FAILED     = 1 << 4,
        };

        uint8_t flags; // if bit 0 is set its a reply, if bit 1 is set we grant credits
        uint8_t senderPe;
        uint8_t senderEp;
        uint8_t replyEp;   // for a normal message this is the reply epId
                           // for a reply this is the enpoint that receives credits
        uint16_t length;
        uint16_t senderVpeId;

        uint64_t replylabel;
    } PACKED;

    struct Header : public ReplyHeader {
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

    static const size_t HEADER_SIZE         = sizeof(Header);
    static const size_t HEADER_COUNT        = 128;
    static const size_t HEADER_REGS         = 2;

    static const epid_t SYSC_SEP            = 0;
    static const epid_t SYSC_REP            = 1;
    static const epid_t UPCALL_REP          = 2;
    static const epid_t DEF_REP             = 3;
    static const epid_t FIRST_FREE_EP       = 4;

    static DTU &get() {
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
        reg_t r1 = read_reg(ep, 1);
        uint16_t cur = r1 & 0xFFFF;
        uint16_t max = (r1 >> 16) & 0xFFFF;
        return cur < max;
    }

    bool has_credits(epid_t ep) const {
        reg_t r1 = read_reg(ep, 1);
        uint16_t cur = r1 & 0xFFFF;
        return cur > 0;
    }

    bool is_valid(epid_t ep) const {
        reg_t r0 = read_reg(ep, 0);
        return static_cast<EpType>(r0 >> 61) != EpType::INVALID;
    }

    reg_t fetch_events() const {
        reg_t old = read_reg(DtuRegs::EVENTS);
        if(old != 0)
            write_reg(CmdRegs::COMMAND, build_command(0, CmdOpCode::ACK_EVENTS, 0, old));
        CPU::memory_barrier();
        return old;
    }

    cycles_t tsc() const {
        return read_reg(DtuRegs::CUR_TIME);
    }
    cycles_t clock() const {
        return read_reg(DtuRegs::CLOCK);
    }

    void print(const char *str, size_t len);

private:
    Errors::Code send(epid_t ep, const void *msg, size_t size, label_t replylbl, epid_t reply_ep);
    Errors::Code reply(epid_t ep, const void *reply, size_t size, const Message *msg);
    Errors::Code read(epid_t ep, void *msg, size_t size, goff_t off, uint flags);
    Errors::Code write(epid_t ep, const void *msg, size_t size, goff_t off, uint flags);

    Message *fetch_msg(epid_t ep) const {
        write_reg(CmdRegs::COMMAND, build_command(ep, CmdOpCode::FETCH_MSG));
        CPU::memory_barrier();
        return reinterpret_cast<Message*>(read_reg(CmdRegs::OFFSET));
    }

    void mark_read(epid_t ep, const Message *msg) {
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
        write_reg(CmdRegs::COMMAND, build_command(0, CmdOpCode::SLEEP, 0, cycles));
        get_error();
    }

    void drop_msgs(epid_t ep, label_t label) {
        // we assume that the one that used the label can no longer send messages. thus, if there are
        // no messages yet, we are done.
        reg_t r0 = read_reg(ep, 0);
        if((r0 & 0x3F) == 0)
            return;

        goff_t base = read_reg(ep, 1);
        size_t bufsize = static_cast<size_t>(1) << ((r0 >> 26) & 0x3F);
        size_t msgsize = (r0 >> 32) & 0xFFFF;
        word_t unread = read_reg(ep, 2) >> 32;
        for(size_t i = 0; i < bufsize; ++i) {
            if(unread & (static_cast<size_t>(1) << i)) {
                m3::DTU::Message *msg = reinterpret_cast<m3::DTU::Message*>(base + (i << msgsize));
                if(msg->label == label)
                    mark_read(ep, msg);
            }
        }
    }

    Errors::Code transfer(reg_t cmd, uintptr_t data, size_t size, goff_t off);

    reg_t get_pfep() const {
        return read_reg(DtuRegs::PF_EP);
    }

    reg_t get_xlate_req() const {
        return read_reg(ReqRegs::XLATE_REQ);
    }
    void set_xlate_req(reg_t val) {
        write_reg(ReqRegs::XLATE_REQ, val);
    }
    void set_xlate_resp(reg_t val) {
        write_reg(ReqRegs::XLATE_RESP, val);
    }

    reg_t get_ext_req() const {
        return read_reg(ReqRegs::EXT_REQ);
    }
    void set_ext_req(reg_t val) {
        write_reg(ReqRegs::EXT_REQ, val);
    }

    static Errors::Code get_error() {
        while(true) {
            reg_t cmd = read_reg(CmdRegs::COMMAND);
            if(static_cast<CmdOpCode>(cmd & 0xF) == CmdOpCode::IDLE)
                return static_cast<Errors::Code>((cmd >> 12) & 0xF);
        }
        UNREACHED;
    }

    static reg_t read_reg(DtuRegs reg) {
        return read_reg(static_cast<size_t>(reg));
    }
    static reg_t read_reg(ReqRegs reg) {
        return read_reg((PAGE_SIZE / sizeof(reg_t)) + static_cast<size_t>(reg));
    }
    static reg_t read_reg(CmdRegs reg) {
        return read_reg(static_cast<size_t>(reg));
    }
    static reg_t read_reg(epid_t ep, size_t idx) {
        return read_reg(DTU_REGS + CMD_REGS + EP_REGS * ep + idx);
    }
    static reg_t read_reg(size_t idx) {
        return CPU::read8b(BASE_ADDR + idx * sizeof(reg_t));
    }

    static void write_reg(DtuRegs reg, reg_t value) {
        write_reg(static_cast<size_t>(reg), value);
    }
    static void write_reg(ReqRegs reg, reg_t value) {
        write_reg((PAGE_SIZE / sizeof(reg_t)) + static_cast<size_t>(reg), value);
    }
    static void write_reg(CmdRegs reg, reg_t value) {
        write_reg(static_cast<size_t>(reg), value);
    }
    static void write_reg(size_t idx, reg_t value) {
        CPU::write8b(BASE_ADDR + idx * sizeof(reg_t), value);
    }

    static void read_header(size_t idx, ReplyHeader &hd) {
        static_assert(sizeof(hd) == 16, "Header size changed");
        uintptr_t base = header_addr(idx);
        uint64_t *words = reinterpret_cast<uint64_t*>(&hd);
        words[0] = CPU::read8b(base);
        words[1] = CPU::read8b(base + 8);
    }
    static void write_header(size_t idx, const ReplyHeader &hd) {
        uintptr_t base = header_addr(idx);
        const uint64_t *words = reinterpret_cast<const uint64_t*>(&hd);
        CPU::write8b(base, words[0]);
        CPU::write8b(base + 8, words[1]);
    }

    static uintptr_t dtu_reg_addr(DtuRegs reg) {
        return BASE_ADDR + static_cast<size_t>(reg) * sizeof(reg_t);
    }
    static uintptr_t dtu_reg_addr(ReqRegs reg) {
        return BASE_ADDR + PAGE_SIZE + static_cast<size_t>(reg) * sizeof(reg_t);
    }
    static uintptr_t cmd_reg_addr(CmdRegs reg) {
        return BASE_ADDR + static_cast<size_t>(reg) * sizeof(reg_t);
    }
    static uintptr_t ep_regs_addr(epid_t ep) {
        return BASE_ADDR + (DTU_REGS + CMD_REGS + ep * EP_REGS) * sizeof(reg_t);
    }
    static uintptr_t header_addr(size_t idx) {
        size_t regCount = DTU_REGS + CMD_REGS + EP_COUNT * EP_REGS;
        return BASE_ADDR + regCount * sizeof(reg_t) + idx * sizeof(ReplyHeader);
    }
    static uintptr_t buffer_addr() {
        size_t regCount = DTU_REGS + CMD_REGS + EP_COUNT * EP_REGS + HD_COUNT * HD_REGS;
        return BASE_ADDR + regCount * sizeof(reg_t);
    }

    static reg_t build_command(epid_t ep, CmdOpCode c, uint flags = 0, reg_t arg = 0) {
        return static_cast<reg_t>(c) |
                (static_cast<reg_t>(ep) << 4) |
                (static_cast<reg_t>(flags) << 11 |
                arg << 16);
    }

    static DTU inst;
};

}
