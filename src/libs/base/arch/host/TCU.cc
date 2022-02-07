/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

#include <base/arch/host/TCUBackend.h>
#include <base/log/Lib.h>
#include <base/util/Math.h>
#include <base/TCU.h>
#include <base/Env.h>
#include <base/Init.h>
#include <base/KIF.h>
#include <base/Panic.h>

#include <sys/types.h>
#include <sys/wait.h>
#include <cstdio>
#include <string.h>
#include <sstream>
#include <signal.h>
#include <unistd.h>

namespace m3 {

INIT_PRIO_TCU TCU TCU::inst;
INIT_PRIO_TCU TCU::Buffer TCU::_buf;

TCU::TCU()
    : _run(true),
      _cmdregs(),
      _epregs(reinterpret_cast<word_t*>(Env::eps_start())),
      _tid() {
    const size_t epsize = EP_REGS * TOTAL_EPS * sizeof(word_t);
    static_assert(epsize <= EPMEM_SIZE, "Not enough space for endpoints");
    memset(const_cast<word_t*>(_epregs), 0, epsize);
}

void TCU::start() {
    _backend = new TCUBackend();

    int res = pthread_create(&_tid, nullptr, thread, this);
    if(res != 0)
        PANIC("pthread_create");
}

void TCU::stop() {
    _run = false;
    // wakeup the thread, if necessary
    _backend->send_command();
}

void TCU::reset() {
    // TODO this is a hack; we cannot leave the recv EPs here in all cases. sometimes the REPs are
    // not inherited so that the child might want to reuse the EP for something else, which does
    // not work, because the cmpxchg fails.
    for(epid_t i = 0; i < TOTAL_EPS; ++i) {
        if(get_ep(i, EP_BUF_ADDR) == 0)
            memset(const_cast<word_t*>(_epregs) + i * EP_REGS, 0, EP_REGS * sizeof(word_t));
    }

    delete _backend;
}

void TCU::configure_recv(epid_t ep, uintptr_t buf, uint order, uint msgorder) {
    set_ep(ep, EP_BUF_ADDR, buf);
    set_ep(ep, EP_BUF_ORDER, order);
    set_ep(ep, EP_BUF_MSGORDER, msgorder);
    set_ep(ep, EP_BUF_ROFF, 0);
    set_ep(ep, EP_BUF_WOFF, 0);
    set_ep(ep, EP_BUF_MSGCNT, 0);
    set_ep(ep, EP_BUF_UNREAD, 0);
    set_ep(ep, EP_BUF_OCCUPIED, 0);
    assert((1UL << (order - msgorder)) <= sizeof(word_t) * 8);
}

Errors::Code TCU::check_cmd(epid_t ep, int op, word_t perms, word_t credits, size_t offset, size_t length) {
    if(op == READ || op == WRITE) {
        if(!(perms & (1U << (op - 1)))) {
            LLOG(TCU, "TCU-error: operation not permitted on ep " << ep << " (perms="
                    << perms << ", op=" << op << ")");
            return Errors::NO_PERM;
        }
        if(offset >= credits || offset + length < offset || offset + length > credits) {
            LLOG(TCU, "TCU-error: invalid parameters (credits=" << credits
                    << ", offset=" << offset << ", datalen=" << length << ")");
            return Errors::INV_ARGS;
        }
    }
    return Errors::NONE;
}

Errors::Code TCU::prepare_reply(epid_t ep, tileid_t &dstpe, epid_t &dstep) {
    const void *src = reinterpret_cast<const void*>(get_cmd(CMD_ADDR));
    const size_t size = get_cmd(CMD_SIZE);
    const size_t reply_off = get_cmd(CMD_OFFSET);
    const word_t bufaddr = get_ep(ep, EP_BUF_ADDR);
    const word_t ord = get_ep(ep, EP_BUF_ORDER);
    const word_t msgord = get_ep(ep, EP_BUF_MSGORDER);

    size_t idx = reply_off >> msgord;
    if(idx >= (1UL << (ord - msgord))) {
        LLOG(TCU, "TCU-error: EP" << ep << ": invalid message offset " << (void*)reply_off);
        return Errors::INV_ARGS;
    }

    Buffer *buf = reinterpret_cast<Buffer*>(const_cast<Message*>(offset_to_msg(bufaddr, reply_off)));
    if(!buf->has_replycap || buf->rpl_ep == TCU::NO_REPLIES) {
        LLOG(TCU, "TCU-error: EP" << ep << ": double-reply for msg " << (void*)reply_off);
        return Errors::INV_ARGS;
    }

    // ack message
    word_t occupied = get_ep(ep, EP_BUF_OCCUPIED);
    // if the slot is not occupied, it's equivalent to the reply EP being invalid
    if(!bit_set(occupied, idx)) {
        LLOG(TCU, "TCU-error: EP" << ep << ": slot not occupied " << (void*)reply_off);
        return Errors::NO_SEP;
    }

    set_bit(occupied, idx, false);
    set_ep(ep, EP_BUF_OCCUPIED, occupied);
    LLOG(TCU, "EP" << ep << ": acked message at index " << idx);

    dstpe = buf->tile;
    dstep = buf->rpl_ep;
    _buf.label = buf->replylabel;
    _buf.credits = 1;
    _buf.crd_ep = buf->snd_ep;
    _buf.length = size;
    if(size > 0)
        memcpy(_buf.data, src, size);
    // invalidate message for replying
    buf->has_replycap = false;
    return Errors::NONE;
}

Errors::Code TCU::prepare_send(epid_t ep, tileid_t &dstpe, epid_t &dstep) {
    const void *src = reinterpret_cast<const void*>(get_cmd(CMD_ADDR));
    const word_t credits = get_ep(ep, EP_CREDITS);
    const word_t msg_order = get_ep(ep, EP_MSGORDER);
    const size_t size = 1UL << msg_order;
    // check if we have enough credits
    if(credits != UNLIM_CREDITS) {
        if(size > credits) {
            LLOG(TCU, "TCU-error: insufficient credits on ep " << ep
                    << " (have #" << fmt(credits, "x") << ", need #" << fmt(size, "x")
                    << ")." << " Ignoring send-command");
            return Errors::NO_CREDITS;
        }
        set_ep(ep, EP_CREDITS, credits - size);
    }
    // check if the message is small enough
    const size_t msg_size = get_cmd(CMD_SIZE) + HEADER_SIZE;
    if(msg_size > size) {
        LLOG(TCUERR, "TCU-error: message too large for ep " << ep
                << " (max #" << fmt(size, "x") << ", need #" << fmt(msg_size, "x")
                << ")." << " Ignoring send-command");
        return Errors::OUT_OF_BOUNDS;
    }

    dstpe = get_ep(ep, EP_PEID);
    dstep = get_ep(ep, EP_EPID);
    _buf.credits = 0;
    _buf.label = get_ep(ep, EP_LABEL);

    _buf.length = get_cmd(CMD_SIZE);
    if(_buf.length > 0)
        memcpy(_buf.data, src, _buf.length);
    return Errors::NONE;
}

Errors::Code TCU::prepare_read(epid_t ep, tileid_t &dstpe, epid_t &dstep) {
    dstpe = get_ep(ep, EP_PEID);
    dstep = get_ep(ep, EP_EPID);

    _buf.credits = 0;
    _buf.label = get_ep(ep, EP_LABEL);
    _buf.length = sizeof(word_t) * 3;
    reinterpret_cast<word_t*>(_buf.data)[0] = get_cmd(CMD_OFFSET);
    reinterpret_cast<word_t*>(_buf.data)[1] = get_cmd(CMD_LENGTH);
    reinterpret_cast<word_t*>(_buf.data)[2] = get_cmd(CMD_ADDR);
    return Errors::NONE;
}

Errors::Code TCU::prepare_write(epid_t ep, tileid_t &dstpe, epid_t &dstep) {
    const void *src = reinterpret_cast<const void*>(get_cmd(CMD_ADDR));
    const size_t size = get_cmd(CMD_SIZE);
    dstpe = get_ep(ep, EP_PEID);
    dstep = get_ep(ep, EP_EPID);

    _buf.credits = 0;
    _buf.label = get_ep(ep, EP_LABEL);
    _buf.length = sizeof(word_t) * 2;
    reinterpret_cast<word_t*>(_buf.data)[0] = get_cmd(CMD_OFFSET);
    reinterpret_cast<word_t*>(_buf.data)[1] = get_cmd(CMD_LENGTH);
    memcpy(_buf.data + _buf.length, src, size);
    _buf.length += size;
    return Errors::NONE;
}

Errors::Code TCU::prepare_ackmsg(epid_t ep) {
    const word_t msgoff = get_cmd(CMD_OFFSET);
    size_t bufaddr = get_ep(ep, EP_BUF_ADDR);
    size_t msgord = get_ep(ep, EP_BUF_MSGORDER);
    size_t ord = get_ep(ep, EP_BUF_ORDER);

    size_t idx = msgoff >> msgord;
    if(idx >= (1UL << (ord - msgord))) {
        LLOG(TCU, "TCU-error: EP" << ep << ": invalid message addr " << (void*)(bufaddr + msgoff));
        return Errors::INV_ARGS;
    }

    word_t occupied = get_ep(ep, EP_BUF_OCCUPIED);
    if(!bit_set(occupied, idx)) {
        LLOG(TCU, "TCU-error: EP" << ep << ": slot at " << (void*)(bufaddr + msgoff) << " not occupied");
        return Errors::INV_ARGS;
    }

    word_t unread = get_ep(ep, EP_BUF_UNREAD);
    set_bit(occupied, idx, false);
    if(bit_set(unread, idx)) {
        set_bit(unread, idx, false);
        set_ep(ep, EP_BUF_UNREAD, unread);
        set_ep(ep, EP_BUF_MSGCNT, get_ep(ep, EP_BUF_MSGCNT) - 1);
        fetched_msg();
    }
    set_ep(ep, EP_BUF_OCCUPIED, occupied);

    LLOG(TCU, "EP" << ep << ": acked message at index " << idx);
    return Errors::NONE;
}

Errors::Code TCU::prepare_fetchmsg(epid_t ep) {
    word_t msgs = get_ep(ep, EP_BUF_MSGCNT);
    if(msgs == 0) {
        set_cmd(CMD_OFFSET, static_cast<word_t>(-1));
        return Errors::NONE;
    }

    size_t roff = get_ep(ep, EP_BUF_ROFF);
    word_t unread = get_ep(ep, EP_BUF_UNREAD);
    size_t ord = get_ep(ep, EP_BUF_ORDER);
    size_t msgord = get_ep(ep, EP_BUF_MSGORDER);
    size_t size = 1UL << (ord - msgord);

    size_t i;
    for(i = roff; i < size; ++i) {
        if(bit_set(unread, i))
            goto found;
    }
    for(i = 0; i < roff; ++i) {
        if(bit_set(unread, i))
            goto found;
    }

    // should not get here
    assert(false);

found:
    assert(bit_set(get_ep(ep, EP_BUF_OCCUPIED), i));

    set_bit(unread, i, false);
    msgs--;
    roff = i + 1;
    assert(Math::bits_set(unread) == msgs);

    LLOG(TCU, "EP" << ep << ": fetched message at index " << i << " (count=" << msgs << ")");

    set_ep(ep, EP_BUF_UNREAD, unread);
    set_ep(ep, EP_BUF_ROFF, roff);
    set_ep(ep, EP_BUF_MSGCNT, msgs);

    set_cmd(CMD_OFFSET, i * (1UL << msgord));

    fetched_msg();

    return Errors::NONE;
}

void TCU::received_msg() {
    _unread_msgs++;
    LLOG(TCU, "TCU: received message");
    if(_sleeping)
        stop_sleep();
}

void TCU::fetched_msg() {
    _unread_msgs--;
    LLOG(TCU, "TCU: fetched message");
}

void TCU::start_sleep() {
    uint64_t timeout = get_cmd(CMD_OFFSET);
    if(_unread_msgs == 0) {
        if(timeout != 0xFFFFFFFFFFFFFFFF)
            _sleep_end = nanotime() + timeout;
        else
            _sleep_end = 0;
        _sleeping = true;
        LLOG(TCU, "TCU: sleep started until " << _sleep_end);
    }
    else {
        // still unread messages -> no sleep. ack is sent if command is ready
        set_cmd(CMD_ERROR, Errors::NONE);
        set_cmd(CMD_CTRL, 0);
    }
}

void TCU::stop_sleep() {
    LLOG(TCU, "TCU: sleep stopped (messages: " << _unread_msgs << ")");
    _sleeping = false;
    // provide feedback to SW
    set_cmd(CMD_ERROR, Errors::NONE);
    set_cmd(CMD_CTRL, 0);
    _backend->send_ack();
}

void TCU::handle_command(tileid_t tile) {
    Errors::Code res = Errors::NONE;
    word_t newctrl = 0;
    tileid_t dstpe;
    epid_t dstep;

    // get regs
    const epid_t ep = get_cmd(CMD_EPID);
    const epid_t reply_ep = get_cmd(CMD_REPLY_EPID);
    const word_t ctrl = get_cmd(CMD_CTRL);
    int op = (ctrl >> OPCODE_SHIFT) & 0xF;
    if(ep >= TOTAL_EPS) {
        LLOG(TCU, "TCU-error: invalid ep-id (" << ep << ")");
        res = Errors::INV_ARGS;
        goto done;
    }

    res = check_cmd(ep, op, get_ep(ep, EP_PERM), get_ep(ep, EP_CREDITS),
                    get_cmd(CMD_OFFSET), get_cmd(CMD_LENGTH));
    if(res != Errors::NONE)
        goto done;

    switch(op) {
        case REPLY:
            res = prepare_reply(ep, dstpe, dstep);
            break;
        case SEND:
            res = prepare_send(ep, dstpe, dstep);
            break;
        case READ:
            res = prepare_read(ep, dstpe, dstep);
            // we report the completion of the read later
            if(res == Errors::NONE)
                newctrl = (ctrl & ~CTRL_START);
            break;
        case WRITE:
            res = prepare_write(ep, dstpe, dstep);
            if(res == Errors::NONE)
                newctrl = (ctrl & ~CTRL_START);
            break;
        case FETCHMSG:
            res = prepare_fetchmsg(ep);
            goto done;
        case ACKMSG:
            res = prepare_ackmsg(ep);
            goto done;
        case SLEEP:
            start_sleep();
            return;
    }
    if(res != Errors::NONE)
        goto done;

    // prepare message (add length and label)
    _buf.opcode = op;
    if(ctrl & CTRL_DEL_REPLY_CAP) {
        _buf.has_replycap = 1;
        _buf.tile = tile;
        _buf.snd_ep = ep;
        _buf.rpl_ep = reply_ep;
        _buf.replylabel = get_cmd(CMD_REPLYLBL);
    }
    else
        _buf.has_replycap = 0;

    if(!send_msg(ep, dstpe, dstep, op == REPLY)) {
        // in case we are doing READ/WRITE, mark the command as finished
        newctrl = 0;
        res = Errors::RECV_GONE;
    }

done:
    set_cmd(CMD_ERROR, static_cast<word_t>(res));
    set_cmd(CMD_CTRL, newctrl);
}

bool TCU::send_msg(epid_t ep, tileid_t dstpe, epid_t dstep, bool isreply) {
    LLOG(TCU, (isreply ? ">> " : "-> ") << fmt(_buf.length, 3) << "b"
            << " lbl=" << fmt(_buf.label, "#0x", sizeof(label_t) * 2)
            << " over " << ep << " to tile:ep=" << dstpe << ":" << dstep
            << " (crd=#" << fmt(get_ep(ep, EP_CREDITS), "x")
            << " rep=" << _buf.rpl_ep << ")");

    return _backend->send(dstpe, dstep, &_buf);
}

void TCU::handle_read_cmd(epid_t ep) {
    word_t base = _buf.label;
    word_t offset = base + reinterpret_cast<word_t*>(_buf.data)[0];
    word_t length = reinterpret_cast<word_t*>(_buf.data)[1];
    word_t dest = reinterpret_cast<word_t*>(_buf.data)[2];
    LLOG(TCU, "(read) " << length << " bytes from #" << fmt(base, "x")
            << "+#" << fmt(offset - base, "x") << " -> " << fmt(dest, "p"));
    tileid_t dstpe = _buf.tile;
    epid_t dstep = _buf.rpl_ep;
    assert(length <= sizeof(_buf.data));

    _buf.opcode = RESP;
    _buf.credits = 0;
    _buf.label = 0;
    _buf.length = sizeof(word_t) * 3;
    reinterpret_cast<word_t*>(_buf.data)[0] = dest;
    reinterpret_cast<word_t*>(_buf.data)[1] = length;
    reinterpret_cast<word_t*>(_buf.data)[2] = 0;
    memcpy(_buf.data + _buf.length, reinterpret_cast<void*>(offset), length);
    _buf.length += length;
    send_msg(ep, dstpe, dstep, true);
}

void TCU::handle_write_cmd(epid_t ep) {
    word_t base = _buf.label;
    word_t offset = base + reinterpret_cast<word_t*>(_buf.data)[0];
    word_t length = reinterpret_cast<word_t*>(_buf.data)[1];
    LLOG(TCU, "(write) " << length << " bytes to #" << fmt(base, "x")
            << "+#" << fmt(offset - base, "x"));
    assert(length <= sizeof(_buf.data));
    tileid_t dstpe = _buf.tile;
    epid_t dstep = _buf.rpl_ep;
    memcpy(reinterpret_cast<void*>(offset), _buf.data + sizeof(word_t) * 2, length);

    _buf.opcode = RESP;
    _buf.credits = 0;
    _buf.label = 0;
    _buf.length = 0;
    send_msg(ep, dstpe, dstep, true);
}

void TCU::handle_resp_cmd() {
    word_t base = _buf.label;
    word_t resp = 0;
    if(_buf.length > 0) {
        word_t offset = base + reinterpret_cast<word_t*>(_buf.data)[0];
        word_t length = reinterpret_cast<word_t*>(_buf.data)[1];
        resp = reinterpret_cast<word_t*>(_buf.data)[2];
        LLOG(TCU, "(resp) " << length << " bytes to #" << fmt(base, "x")
                << "+#" << fmt(offset - base, "x") << " -> " << resp);
        assert(length <= sizeof(_buf.data));
        memcpy(reinterpret_cast<void*>(offset), _buf.data + sizeof(word_t) * 3, length);
    }
    /* provide feedback to SW */
    set_cmd(CMD_CTRL, resp);
    _backend->send_ack();
}

void TCU::handle_msg(size_t len, epid_t ep) {
    const size_t msgord = get_ep(ep, EP_BUF_MSGORDER);
    const size_t msgsize = 1UL << msgord;
    if(len > msgsize) {
        LLOG(TCUERR, "TCU-error: dropping message for EP " << ep
                << " because space is not sufficient"
                << " (required: " << len << ", available: " << msgsize << ")");
        return;
    }

    word_t occupied = get_ep(ep, EP_BUF_OCCUPIED);
    word_t unread = get_ep(ep, EP_BUF_UNREAD);
    word_t msgs = get_ep(ep, EP_BUF_MSGCNT);
    size_t woff = get_ep(ep, EP_BUF_WOFF);
    size_t ord = get_ep(ep, EP_BUF_ORDER);
    size_t size = 1UL << (ord - msgord);

    size_t i;
    for (i = woff; i < size; ++i)
    {
        if (!bit_set(occupied, i))
            goto found;
    }
    for (i = 0; i < woff; ++i)
    {
        if (!bit_set(occupied, i))
            goto found;
    }

    LLOG(TCUERR, "EP" << ep << ": dropping message because no slot is free");
    return;

found:
    set_bit(occupied, i, true);
    set_bit(unread, i, true);
    msgs++;
    woff = i + 1;
    assert(Math::bits_set(unread) == msgs);

    LLOG(TCU, "EP" << ep << ": put message at index " << i << " (count=" << msgs << ")");

    set_ep(ep, EP_BUF_OCCUPIED, occupied);
    set_ep(ep, EP_BUF_UNREAD, unread);
    set_ep(ep, EP_BUF_MSGCNT, msgs);
    set_ep(ep, EP_BUF_WOFF, woff);

    auto msg = const_cast<Message*>(offset_to_msg(get_ep(ep, EP_BUF_ADDR), i * (1UL << msgord)));
    memcpy(msg, &_buf, len);

    received_msg();
}

bool TCU::handle_receive(epid_t ep) {
    ssize_t res = _backend->recv(ep, &_buf);
    if(res < 0)
        return false;

    const int op = _buf.opcode;
    switch(op) {
        case READ:
            handle_read_cmd(ep);
            break;
        case RESP:
            handle_resp_cmd();
            break;
        case WRITE:
            handle_write_cmd(ep);
            break;
        case SEND:
        case REPLY:
            handle_msg(static_cast<size_t>(res), ep);
            break;
    }

    // refill credits
    if(_buf.crd_ep >= TOTAL_EPS)
        LLOG(TCUERR, "TCU-error: should give credits to endpoint " << _buf.crd_ep);
    else {
        word_t credits = get_ep(_buf.crd_ep, EP_CREDITS);
        word_t msg_order = get_ep(_buf.crd_ep, EP_MSGORDER);
        if(_buf.credits && credits != UNLIM_CREDITS) {
            LLOG(TCU, "Refilling credits of ep " << _buf.crd_ep
                << " from #" << fmt(credits, "x") << " to #" << fmt(credits + (1UL << msg_order), "x"));
            set_ep(_buf.crd_ep, EP_CREDITS, credits + (1UL << msg_order));
        }
    }

    LLOG(TCU, "<- " << fmt(static_cast<size_t>(res) - HEADER_SIZE, 3)
           << "b lbl=" << fmt(_buf.label, "#0x", sizeof(label_t) * 2)
           << " ep=" << ep
           << " (cnt=#" << fmt(get_ep(ep, EP_BUF_MSGCNT), "x") << ","
           << "crd=#" << fmt(get_ep(ep, EP_CREDITS), "x") << ")");
    return true;
}

Errors::Code TCU::perform_transfer(epid_t ep, uintptr_t data_addr, size_t size,
                                   goff_t off, int cmd) {
    while(size > 0) {
        size_t amount = Math::min(size, PAGE_SIZE - (data_addr & PAGE_MASK));
        setup_command(ep, cmd, reinterpret_cast<const void*>(data_addr), amount, off,
                      amount, label_t(), 0);
        auto res = exec_command();
        if(res != Errors::NONE)
            return res;

        size -= amount;
        data_addr += amount;
        off += amount;
    }
    return Errors::NONE;
}

Errors::Code TCU::exec_command() {
    _backend->send_command();
    while(!_backend->recv_ack())
        sleep();
    assert(is_ready());
    return static_cast<Errors::Code>(get_cmd(CMD_ERROR));
}

bool TCU::receive_knotify(int *pid, int *status) {
    return _backend->receive_knotify(pid, status);
}

static volatile int childs = 0;

static void sigchild(int) {
    childs++;
    signal(SIGCLD, sigchild);
}

void *TCU::thread(void *arg) {
    TCU *dma = static_cast<TCU*>(arg);
    tileid_t tile = env()->tile_id;

    if(tile != 0)
        signal(SIGCLD, sigchild);
    else
        dma->_backend->bind_knotify();

    while(dma->_run) {
        // notify kernel about exited childs
        while(childs > 0) {
            int status;
            int pid = ::wait(&status);
            if(pid != -1)
                dma->_backend->notify_kernel(pid, status);
            childs--;
        }

        // should we send something?
        if(dma->_backend->recv_command()) {
            assert((dma->get_cmd(CMD_CTRL) & CTRL_START) != 0);
            dma->handle_command(tile);
            if(dma->is_ready())
                dma->_backend->send_ack();
        }

        // have we received a message?
        for(epid_t ep = 0; ep < TOTAL_EPS; ++ep)
            dma->handle_receive(ep);

        auto now = dma->nanotime();
        if(dma->_sleeping && dma->_sleep_end != 0 && now >= dma->_sleep_end)
            dma->stop_sleep();

        uint64_t timeout = 0;
        if(dma->_sleeping && dma->_sleep_end != 0)
            timeout = dma->_sleep_end - now;
        dma->_backend->wait_for_work(timeout);
    }

    // deny further receives
    dma->_backend->shutdown();

    // handle all outstanding messages
    while(1) {
        bool received = false;
        for(epid_t ep = 0; ep < TOTAL_EPS; ++ep)
            received |= dma->handle_receive(ep);
        if(!received)
            break;
    }

    delete dma->_backend;
    return 0;
}

}
