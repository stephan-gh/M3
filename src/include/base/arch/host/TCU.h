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
#include <base/util/String.h>
#include <base/util/Util.h>
#include <base/Errors.h>
#include <base/Env.h>

#include <assert.h>
#include <iomanip>
#include <limits>
#include <ostream>
#include <pthread.h>
#include <time.h>
#include <unistd.h>

#define TOTAL_EPS            128
#define AVAIL_EPS            TOTAL_EPS

namespace m3 {

class Gate;
class TCUBackend;

class TCU {
    friend class Gate;
    friend class MsgBackend;
    friend class SocketBackend;

    static constexpr size_t MAX_DATA_SIZE   = 1 * 1024 * 1024;
public:
    typedef word_t reg_t;

    struct Header {
        size_t length;          // = mtype -> has to be non-zero
        unsigned char opcode;   // should actually be part of length but causes trouble in msgsnd
        label_t label;
        uint8_t has_replycap;
        uint16_t pe;
        uint8_t rpl_ep;
        uint8_t snd_ep;
        label_t replylabel;
        uint8_t credits;
        uint8_t crd_ep;
    } PACKED;

    struct Buffer : public Header {
        char data[MAX_DATA_SIZE];
    };

    struct Message : public Header {
        epid_t send_ep() const {
            return snd_ep;
        }
        epid_t reply_ep() const {
            return rpl_ep;
        }

        unsigned char data[];
    } PACKED;

    static constexpr size_t HEADER_SIZE         = sizeof(Buffer) - MAX_DATA_SIZE;
    static const size_t HEADER_COUNT            = std::numeric_limits<size_t>::max();

    static constexpr size_t MAX_MSGS            = sizeof(word_t) * 8;

    static const reg_t INVALID_EP               = 0xFF;
    static const size_t NO_REPLIES              = INVALID_EP;
    static const reg_t UNLIM_CREDITS            = 0xFFFFFFFF;

    // command registers
    static constexpr size_t CMD_ADDR            = 0;
    static constexpr size_t CMD_SIZE            = 1;
    static constexpr size_t CMD_EPID            = 2;
    static constexpr size_t CMD_CTRL            = 3;
    static constexpr size_t CMD_OFFSET          = 4;
    static constexpr size_t CMD_REPLYLBL        = 5;
    static constexpr size_t CMD_REPLY_EPID      = 6;
    static constexpr size_t CMD_LENGTH          = 7;
    static constexpr size_t CMD_ERROR           = 8;

    // register starts and counts (cont.)
    static constexpr size_t CMDS_RCNT           = 1 + CMD_ERROR;

    static constexpr size_t EP_VALID            = 0;

    // receive buffer registers
    static constexpr size_t EP_BUF_ADDR         = 1;
    static constexpr size_t EP_BUF_ORDER        = 2;
    static constexpr size_t EP_BUF_MSGORDER     = 3;
    static constexpr size_t EP_BUF_ROFF         = 4;
    static constexpr size_t EP_BUF_WOFF         = 5;
    static constexpr size_t EP_BUF_MSGCNT       = 6;
    static constexpr size_t EP_BUF_MSGQID       = 7;
    static constexpr size_t EP_BUF_UNREAD       = 8;
    static constexpr size_t EP_BUF_OCCUPIED     = 9;

    // for sending message and accessing memory
    static constexpr size_t EP_PEID             = 10;
    static constexpr size_t EP_EPID             = 11;
    static constexpr size_t EP_LABEL            = 12;
    static constexpr size_t EP_CREDITS          = 13;
    static constexpr size_t EP_MSGORDER         = 14;
    static constexpr size_t EP_PERM             = 15;

    // bits in ctrl register
    static constexpr word_t CTRL_START          = 0x1;
    static constexpr word_t CTRL_DEL_REPLY_CAP  = 0x2;

    static constexpr size_t OPCODE_SHIFT        = 3;

    // register counts (cont.)
    static constexpr size_t EP_REGS            = 1 + EP_PERM;

    enum CmdFlags {
        NOPF                                    = 1,
    };

    enum Op {
        READ                                    = 1,
        WRITE                                   = 2,
        SEND                                    = 3,
        REPLY                                   = 4,
        RESP                                    = 5,
        FETCHMSG                                = 6,
        ACKMSG                                  = 7,
    };

    static const epid_t PEXUP_REP               = 0;    // unused

    static const epid_t SYSC_SEP_OFF            = 0;
    static const epid_t SYSC_REP_OFF            = 1;
    static const epid_t UPCALL_REP_OFF          = 2;
    static const epid_t DEF_REP_OFF             = 3;
    static const epid_t PG_SEP_OFF              = 0;    // unused
    static const epid_t PG_REP_OFF              = 0;    // unused

    static const epid_t FIRST_USER_EP           = 0;
    static const epid_t STD_EPS_COUNT           = 4;

    static TCU &get() {
        return inst;
    }

    explicit TCU();

    void reset();

    word_t get_cmd(size_t reg) const {
        return _cmdregs[reg];
    }
    void set_cmd(size_t reg, word_t val) {
        _cmdregs[reg] = val;
    }

    word_t *ep_regs() {
        return const_cast<word_t*>(_epregs);
    }

    word_t get_ep(epid_t ep, size_t reg) const {
        return _epregs[ep * EP_REGS + reg];
    }
    void set_ep(epid_t ep, size_t reg, word_t val) {
        _epregs[ep * EP_REGS + reg] = val;
    }

    void configure(epid_t ep, label_t label, uint perms, peid_t pe, epid_t dstep,
                   word_t credits, uint msgorder) {
        configure(const_cast<word_t*>(_epregs), ep, label, perms, pe, dstep, credits, msgorder);
    }
    static void configure(word_t *eps, epid_t ep, label_t label, uint perms, peid_t pe,
                          epid_t dstep, word_t credits, uint msgorder) {
        eps[ep * EP_REGS + EP_VALID] = 1;
        eps[ep * EP_REGS + EP_LABEL] = label;
        eps[ep * EP_REGS + EP_PEID] = pe;
        eps[ep * EP_REGS + EP_EPID] = dstep;
        eps[ep * EP_REGS + EP_CREDITS] = credits;
        eps[ep * EP_REGS + EP_MSGORDER] = msgorder;
        eps[ep * EP_REGS + EP_PERM] = perms;
    }

    void configure_recv(epid_t ep, uintptr_t buf, uint order, uint msgorder);

    Errors::Code send(epid_t ep, const MsgBuf &msg, label_t replylbl, epid_t replyep) {
        setup_command(ep, SEND, msg.bytes(), msg.size(), 0, 0, replylbl, replyep);
        return exec_command();
    }
    Errors::Code send_aligned(epid_t ep, const void *msg, size_t len, label_t replylbl, epid_t replyep) {
        setup_command(ep, SEND, msg, len, 0, 0, replylbl, replyep);
        return exec_command();
    }
    Errors::Code reply(epid_t ep, const MsgBuf &reply, size_t msg_off) {
        setup_command(ep, REPLY, reply.bytes(), reply.size(), msg_off, 0, label_t(), 0);
        return exec_command();
    }
    Errors::Code read(epid_t ep, void *msg, size_t size, size_t off) {
        setup_command(ep, READ, msg, size, off, size, label_t(), 0);
        return exec_command();
    }
    Errors::Code write(epid_t ep, const void *msg, size_t size, size_t off) {
        setup_command(ep, WRITE, msg, size, off, size, label_t(), 0);
        return exec_command();
    }

    bool is_valid(epid_t ep) const {
        return get_ep(ep, EP_VALID) == 1;
    }
    bool has_missing_credits(epid_t) const {
        // TODO not supported
        return false;
    }

    uint credits(epid_t ep) const {
        return get_ep(ep, EP_CREDITS) >> get_ep(ep, EP_MSGORDER);
    }

    bool has_msgs(epid_t ep) const {
        return get_ep(ep, EP_BUF_UNREAD) != 0;
    }

    size_t fetch_msg(epid_t ep) {
        if(get_ep(ep, EP_BUF_MSGCNT) == 0)
            return static_cast<size_t>(-1);

        set_cmd(CMD_EPID, ep);
        set_cmd(CMD_CTRL, (FETCHMSG << OPCODE_SHIFT) | CTRL_START);
        exec_command();
        return get_cmd(CMD_OFFSET);
    }

    Errors::Code ack_msg(epid_t ep, size_t msg_off) {
        set_cmd(CMD_EPID, ep);
        set_cmd(CMD_OFFSET, msg_off);
        set_cmd(CMD_CTRL, (ACKMSG << OPCODE_SHIFT) | CTRL_START);
        return exec_command();
    }

    bool is_ready() const {
        return (get_cmd(CMD_CTRL) >> OPCODE_SHIFT) == 0;
    }

    void setup_command(epid_t ep, int op, const void *msg, size_t size, size_t offset,
                       size_t len, label_t replylbl, epid_t replyep) {
        set_cmd(CMD_ADDR, reinterpret_cast<word_t>(msg));
        set_cmd(CMD_SIZE, size);
        set_cmd(CMD_EPID, ep);
        set_cmd(CMD_OFFSET, offset);
        set_cmd(CMD_LENGTH, len);
        set_cmd(CMD_REPLYLBL, replylbl);
        set_cmd(CMD_REPLY_EPID, replyep);
        set_cmd(CMD_ERROR, 0);
        if(op == REPLY)
            set_cmd(CMD_CTRL, static_cast<word_t>(op << OPCODE_SHIFT) | CTRL_START);
        else
            set_cmd(CMD_CTRL, static_cast<word_t>(op << OPCODE_SHIFT) | CTRL_START | CTRL_DEL_REPLY_CAP);
    }

    Errors::Code exec_command();

    bool receive_knotify(pid_t *pid, int *status);

    void start();
    void stop();
    pthread_t tid() const {
        return _tid;
    }

    uint64_t nanotime() const {
        struct timespec ts;
        clock_gettime(CLOCK_REALTIME, &ts);
        return static_cast<cycles_t>(ts.tv_sec) * 1000000000 + static_cast<cycles_t>(ts.tv_nsec);
    }

    void sleep() const {
        usleep(1);
    }
    void wait_for_msg(epid_t) const {
        sleep();
    }

    void drop_msgs(size_t buf_addr, epid_t ep, label_t label) {
        // we assume that the one that used the label can no longer send messages. thus, if there are
        // no messages yet, we are done.
        if(get_ep(ep, m3::TCU::EP_BUF_MSGCNT) == 0)
            return;

        int order = static_cast<int>(get_ep(ep, m3::TCU::EP_BUF_ORDER));
        int msgorder = static_cast<int>(get_ep(ep, m3::TCU::EP_BUF_MSGORDER));
        word_t unread = get_ep(ep, m3::TCU::EP_BUF_UNREAD);
        int max = 1 << (order - msgorder);
        for(int i = 0; i < max; ++i) {
            if(unread & (1UL << i)) {
                size_t msg_off = static_cast<size_t>(i) << msgorder;
                const Message *msg = offset_to_msg(buf_addr, msg_off);
                if(msg->label == label)
                    ack_msg(ep, msg_off);
            }
        }
    }

    static size_t msg_to_offset(size_t base, const Message *msg) {
        return reinterpret_cast<uintptr_t>(msg) - (base + env()->rbuf_start());
    }
    static const Message *offset_to_msg(size_t base, size_t msg_off) {
        return reinterpret_cast<const Message*>(base + env()->rbuf_start() + msg_off);
    }

private:
    bool bit_set(word_t mask, size_t idx) const {
        return mask & (static_cast<word_t>(1) << idx);
    }
    void set_bit(word_t &mask, size_t idx, bool set) {
        if(set)
            mask |= static_cast<word_t>(1) << idx;
        else
            mask &= ~(static_cast<word_t>(1) << idx);
    }

    Errors::Code prepare_reply(epid_t ep, peid_t &dstpe, epid_t &dstep);
    Errors::Code prepare_send(epid_t ep, peid_t &dstpe, epid_t &dstep);
    Errors::Code prepare_read(epid_t ep, peid_t &dstpe, epid_t &dstep);
    Errors::Code prepare_write(epid_t ep, peid_t &dstpe, epid_t &dstep);
    Errors::Code prepare_fetchmsg(epid_t ep);
    Errors::Code prepare_ackmsg(epid_t ep);

    bool send_msg(epid_t ep, peid_t dstpe, epid_t dstep, bool isreply);
    void handle_read_cmd(epid_t ep);
    void handle_write_cmd(epid_t ep);
    void handle_resp_cmd();
    void handle_command(peid_t pe);
    void handle_msg(size_t len, epid_t ep);
    bool handle_receive(epid_t ep);

    static Errors::Code check_cmd(epid_t ep, int op, word_t addr, word_t credits,
                                  size_t offset, size_t length);
    static void *thread(void *arg);

    volatile bool _run;
    volatile word_t _cmdregs[CMDS_RCNT];
    volatile word_t *_epregs;
    TCUBackend *_backend;
    pthread_t _tid;
    static Buffer _buf;
    static TCU inst;
};

}
