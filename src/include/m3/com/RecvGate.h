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

#include <m3/com/Gate.h>
#include <m3/WorkLoop.h>

#include <functional>
#include <memory>

namespace m3 {

class GateIStream;
class SendGate;
class VPE;
template<class HDL>
class Server;

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
    friend class Pager;
    template<class HDL>
    friend class Server;
    friend class AladdinAccel;
    friend class InDirAccel;
    friend class StreamAccel;

    class RecvGateWorkItem : public WorkItem {
    public:
        explicit RecvGateWorkItem(RecvGate *gate) noexcept : _gate(gate) {
        }

        virtual void work() override;

    protected:
        RecvGate *_gate;
    };

    explicit RecvGate(capsel_t cap, uint order, uint msgorder, uint flags) noexcept
        : Gate(RECV_GATE, cap, flags),
          _buf(),
          _order(order),
          _msgorder(msgorder),
          _free_buf(),
          _handler(),
          _workitem() {
    }
    explicit RecvGate(capsel_t cap, epid_t ep, void *buf, uint order, uint msgorder, uint flags);

public:
    using msghandler_t = std::function<void(GateIStream&)>;

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
    static RecvGate create(uint order, uint msgorder);
    /**
     * Creates a new receive gate at selector <sel> with given size.
     *
     * @param sel the capability selector to use
     * @param order the size of the buffer (2^<order> bytes)
     * @param msgorder the size of messages within the buffer (2^<msgorder> bytes)
     * @param flags the flags to control whether the cap is kept
     * @return the receive gate
     */
    static RecvGate create(capsel_t sel, uint order, uint msgorder, uint flags = 0);

    /**
     * Binds the receive gate at selector <sel>.
     *
     * @param sel the capability selector
     * @param order the size of the buffer (2^<order> bytes)
     * @param msgorder the size of messages within the buffer (2^<msgorder> bytes)
     * @return the receive gate
     */
    static RecvGate bind(capsel_t sel, uint order, uint msgorder) noexcept;

    RecvGate(const RecvGate&) = delete;
    RecvGate &operator=(const RecvGate&) = delete;
    RecvGate(RecvGate &&r) noexcept
            : Gate(std::move(r)),
              _buf(r._buf),
              _order(r._order),
              _free_buf(r._free_buf),
              _handler(r._handler),
              _workitem(std::move(r._workitem)) {
        r._free_buf = false;
        r._workitem = nullptr;
    }
    ~RecvGate();

    /**
     * @return the buffer address
     */
    const void *addr() const noexcept {
        return _buf;
    }

    /**
     * @return the number of slots in the receive buffer
     */
    uint slots() const noexcept {
        return 1U << (_order - _msgorder);
    }

    /**
     * Activates this receive gate, i.e., lets the kernel configure a free endpoint for it.
     */
    void activate();

    /**
     * Activates this receive gate on the given endpoint with given receive buffer address. This
     * call is intended for CUs that don't manage their own receive buffer space. For that reason,
     * the receive buffer addresses needs to be chosen externally.
     *
     * @param ep the endpoint
     * @param addr the receive buffer address
     */
    void activate_on(const EP &ep, uintptr_t addr);

    /**
     * Deactivates and stops the receive gate.
     */
    void deactivate() noexcept;

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
     * Fetches a message from this receive gate and returns it, if there is any.
     *
     * @return the message or nullptr
     */
    const TCU::Message *fetch();

    /**
     * Waits until a message is received. If <sgate> is given, it will stop if as soon as <sgate>
     * gets invalid and throw an exception.
     *
     * @param sgate the send gate (optional), if waiting for a reply
     * @return the fetched message
     */
    const TCU::Message *receive(SendGate *sgate);

    /**
     * Replies the <len> bytes at <reply> to the message <msg>.
     *
     * @param reply the reply message to send
     * @param len the length of the data
     * @param msg the message to reply to
     */
    void reply(const void *reply, size_t len, const TCU::Message *msg);

    /**
     * Marks the given message as 'read', allowing the TCU to overwrite it with a new message.
     *
     * @param msg the message
     */
    void ack_msg(const TCU::Message *msg);

    /**
     * Drops all messages with given label. That is, these messages will be marked as read.
     *
     * @param label the label
     */
    void drop_msgs_with(label_t label) noexcept;

private:
    void set_ep(epid_t ep) {
        Gate::set_ep(new EP(EP::bind(ep)));
    }

    static void *allocate(VPE &vpe, size_t size);
    static void free(void *);

    void *_buf;
    uint _order;
    uint _msgorder;
    bool _free_buf;
    msghandler_t _handler;
    std::unique_ptr<RecvGateWorkItem> _workitem;
    static RecvGate _syscall;
    static RecvGate _upcall;
    static RecvGate _default;
};

}
