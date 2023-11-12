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

#pragma once

#include <base/Common.h>
#include <base/util/Util.h>

#include <m3/WorkLoop.h>
#include <m3/com/Gate.h>

#include <functional>
#include <memory>

namespace m3 {

class GateIStream;
class MemGate;
class SendGate;
class Activity;
template<class HDL>
class Server;
class RecvBuf;
class EnvUserBackend;

/**
 * A receive capability is the precursor of a RecvGate.
 *
 * RecvCap can be turned into a RecvGate through activation.
 */
class RecvCap : public ObjCap {
    explicit RecvCap(capsel_t sel, uint order, uint msgorder, uint flags, bool create);

public:
    /**
     * Creates a new receive capability with given size.
     *
     * @param order the size of the buffer (2^<order> bytes)
     * @param msgorder the size of messages within the buffer (2^<msgorder> bytes)
     * @return the receive capability
     */
    static RecvCap create(uint order, uint msgorder);
    /**
     * Creates a new receive capability at selector <sel> with given size.
     *
     * @param sel the capability selector to use
     * @param order the size of the buffer (2^<order> bytes)
     * @param msgorder the size of messages within the buffer (2^<msgorder> bytes)
     * @return the receive capability
     */
    static RecvCap create(capsel_t sel, uint order, uint msgorder);

    /**
     * Creates the receive capability with given name as defined in the application's configuration.
     *
     * @param name the name in the configuration file
     * @return the receive capability
     */
    static RecvCap create_named(const char *name);

    /**
     * Binds the receive capability at selector <sel>.
     *
     * @param sel the capability selector
     * @return the receive capability
     */
    static RecvCap bind(capsel_t sel) noexcept;

    RecvCap(const RecvCap &) = delete;
    RecvCap &operator=(const RecvCap &) = delete;
    RecvCap(RecvCap &&r) noexcept : ObjCap(std::move(r)), _order(r._order), _msgorder(r._msgorder) {
    }

    /**
     * @return the number of slots in the receive buffer
     */
    uint slots() const {
        fetch_buffer_size();
        return 1U << (_order - _msgorder);
    }

    /**
     * Activates this RecvCap and thereby turns it into a usable RecvGate
     *
     * This will allocate a new EP from the EPMng.
     *
     * @return the created RecvGate
     */
    RecvGate activate();

    /**
     * Activates this receive gate on the given endpoint with given receive buffer address. This
     * call is intended for CUs that don't manage their own receive buffer space. For that reason,
     * the receive buffer addresses needs to be chosen externally.
     *
     * @param ep the endpoint
     * @param mem the receive buffer (nullptr for SPM)
     * @param off the offset within the buffer
     */
    void activate_on(const EP &ep, MemGate *mem, size_t off);

private:
    void fetch_buffer_size() const;

    mutable uint _order;
    mutable uint _msgorder;
};

/**
 * A receive gate is used to receive messages from send gates. To this end, it has a receive buffer
 * of a fixed message and total size. Multiple send gates can be created for one receive gate. After
 * a message has been received, the reply operation can be used to send a reply back to the sender.
 *
 * Receiving messages is possible by waiting for them using the wait() method. This approach is used
 * when, e.g., receiving a reply upon a sent message. Alternatively, one can start to listen to
 * received messages. In this case, a WorkLoop item is created.
 */
class RecvGate : public Gate {
    typedef RecvCap Cap;

    friend class RecvCap;
    friend class Pager;
    template<class HDL>
    friend class Server;
    friend class AladdinAccel;
    friend class InDirAccel;
    friend class StreamAccel;
    friend class EnvUserBackend;

    class RecvGateWorkItem : public WorkItem {
    public:
        explicit RecvGateWorkItem(RecvGate *gate) noexcept : _gate(gate) {
        }

        virtual void work() override;

    protected:
        RecvGate *_gate;
    };

    explicit RecvGate(capsel_t cap, size_t addr, RecvBuf *buf, EP *ep, uint order, uint msgorder,
                      uint flags) noexcept;

public:
    using msghandler_t = std::function<void(GateIStream &)>;

    /**
     * @return the receive gate for system call replies
     */
    static RecvGate &syscall() noexcept {
        return _syscall;
    }
    /**
     * @return the receive gate for upcalls
     */
    static RecvGate &upcall() noexcept {
        return _upcall;
    }
    /**
     * @return the default receive gate. can be used whenever a buffer for a single message with a
     *  reasonable size is sufficient
     */
    static RecvGate &def() noexcept {
        return _default;
    }

    /**
     * Creates a new receive gate with given size.
     *
     * @param order the size of the buffer (2^<order> bytes)
     * @param msgorder the size of messages within the buffer (2^<msgorder> bytes)
     * @return the receive gate
     */
    static RecvGate create(uint order, uint msgorder) {
        return RecvCap::create(order, msgorder).activate();
    }
    /**
     * Creates a new receive gate at selector <sel> with given size.
     *
     * @param sel the capability selector to use
     * @param order the size of the buffer (2^<order> bytes)
     * @param msgorder the size of messages within the buffer (2^<msgorder> bytes)
     * @return the receive gate
     */
    static RecvGate create(capsel_t sel, uint order, uint msgorder) {
        return RecvCap::create(sel, order, msgorder).activate();
    }

    /**
     * Creates the receive gate with given name as defined in the application's configuration.
     *
     * @param name the name in the configuration file
     * @param replygate the receive gate to which the replies should be sent
     * @return the receive gate
     */
    static RecvGate create_named(const char *name) {
        return RecvCap::create_named(name).activate();
    }

    /**
     * Binds the receive gate at selector <sel>.
     *
     * @param sel the capability selector
     * @return the receive gate
     */
    static RecvGate bind(capsel_t sel) {
        return RecvCap::bind(sel).activate();
    }

    RecvGate(const RecvGate &) = delete;
    RecvGate &operator=(const RecvGate &) = delete;
    RecvGate(RecvGate &&r) noexcept
        : Gate(std::move(r)),
          _buf(r._buf),
          _buf_addr(r._buf_addr),
          _order(r._order),
          _msgorder(r._msgorder),
          _handler(r._handler),
          _workitem(std::move(r._workitem)) {
        r._buf = nullptr;
        r._workitem = nullptr;
    }
    ~RecvGate();

    /**
     * @return the address of the receive buffer (or 0 if not activated)
     */
    uintptr_t address() const noexcept {
        return _buf_addr;
    }

    /**
     * @return the number of slots in the receive buffer
     */
    uint slots() const noexcept {
        return 1U << (_order - _msgorder);
    }

    /**
     * Starts to listen for received messages, i.e., adds an item to the given workloop.
     *
     * @param wl the workloop
     * @param handler the handler to call for received messages
     */
    void start(WorkLoop *wl, msghandler_t handler);

    /**
     * Stops to listen for received messages
     */
    void stop() noexcept;

    /**
     * Checks whether unread messages are available without fetching them
     *
     * @return true if there are unread messages
     */
    bool has_msgs() noexcept;

    /**
     * Suspend the activity until a message arrives on this RecvGate.
     */
    void wait_for_msg();

    /**
     * Fetches a message from this receive gate and returns it, if there is any.
     *
     * @return the message or nullptr
     */
    const TCU::Message *fetch() noexcept;

    /**
     * Waits until a message is received. If <sgate> is given, it will stop if as soon as <sgate>
     * gets invalid and throw an exception.
     *
     * @param sgate the send gate (optional), if waiting for a reply
     * @return the fetched message
     */
    const TCU::Message *receive(SendGate *sgate);

    /**
     * Sends <reply> as a reply to the message <msg>.
     *
     * @param reply the reply message to send
     * @param msg the message to reply to
     */
    void reply(const MsgBuf &reply, const TCU::Message *msg) {
        reply_aligned(reply.bytes(), reply.size(), msg);
    }

    /**
     * Sends <reply> as a reply to the message <msg>, assuming that <reply> is properly aligned. The
     * message address needs to be 16-byte aligned and the message cannot contain a page boundary.
     *
     * @param reply the reply message to send
     * @param len the length of the reply
     * @param msg the message to reply to
     */
    void reply_aligned(const void *reply, size_t len, const TCU::Message *msg);

    /**
     * Marks the given message as 'read', allowing the TCU to overwrite it with a new message.
     *
     * @param msg the message
     */
    void ack_msg(const TCU::Message *msg) noexcept;

    /**
     * Drops all messages with given label. That is, these messages will be marked as read.
     *
     * @param label the label
     */
    void drop_msgs_with(label_t label) noexcept;

private:
    RecvBuf *_buf;
    size_t _buf_addr;
    uint _order;
    uint _msgorder;
    msghandler_t _handler;
    std::unique_ptr<RecvGateWorkItem> _workitem;
    static RecvGate _syscall;
    static RecvGate _upcall;
    static RecvGate _default;
};

}
