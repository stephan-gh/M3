/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
 * Copyright (C) 2017, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
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

#include <base/col/List.h>
#include <base/col/Treap.h>
#include <base/util/Reference.h>

#include <m3/netrs/DataQueue.h>
#include <m3/netrs/Net.h>

namespace m3 {

class NetworkManagerRs;

class SocketRs : public m3::TreapNode<SocketRs, int>, public RefCounted {
    friend class NetworkManagerRs;

    static const int EVENT_FETCH_BATCH_SIZE = 4;

public:
    enum State {
        Bound,
        Listening,
        Connecting,
        Connected,
        Closed
    };

    virtual ~SocketRs() {}

    int sd() const noexcept {
        return _sd;
    }

    State state() const noexcept {
        return _state;
    }

    bool blocking() const noexcept {
        return _blocking;
    }

    /**
     * Determines whether socket operates in blocking or non-blocking mode.
     *
     * When a socket operates in blocking mode operations on the socket block the caller
     * until a result is present (e.g. a connection request is present in Socket::accept,
     * or data is available in Socket::recv).
     * A socket in non-blocking mode returns Errors::WOULD_BLOCK instead of blocking the caller.
     *
     * Sockets in blocking mode require the application to use a multithreaded workloop.
     *
     * @param blocking whether socket operates in blocking or
     *        non-blocking mode (default = blocking)
     */
    void blocking(bool blocking) noexcept {
        _blocking = blocking;
    }

    /**
     * Sends at most <amount> bytes from <src> to the socket at <addr>:<port>.
     *
     * @param src the data to send
     * @param amount the number of bytes to send
     * @param dst_addr destination socket address
     * @param dst_port destination socket port
     * @return the number of sent bytes (-1 if it would block and the socket is non-blocking)
     */
    virtual ssize_t sendto(const void *src, size_t amount, IpAddr dst_addr, uint16_t dst_port);

    /**
     * Receives at most <amount> bytes into <src> and returns the number of received bytes.
     *
     * @param dst the destination buffer
     * @param amount the maximum number of bytes to receive
     * @return the number of received bytes (-1 if it would block and the socket is non-blocking)
     */
    ssize_t recv(void *dst, size_t amount) {
        return recvfrom(dst, amount, nullptr, nullptr);
    }

    /**
     * Receives <amount> or a smaller number of bytes into <dst>.
     *
     * @param dst the destination buffer
     * @param amount the number of bytes to receive
     * @param src_addr if not null, the source address is filled in
     * @param src_port if not null, the source port is filled in
     * @return the number of received bytes (-1 if it would block and the socket is non-blocking)
     */
    virtual ssize_t recvfrom(void *dst, size_t amount, IpAddr *src_addr, uint16_t *src_port);

    /**
     * Performs a hard abort by closing the socket on our end and dropping all data. Note that
     * submitted packets for sending are not guaranteed to be sent out.
     */
    void abort();

    void wait_for_event();
    void process_events();

protected:
    explicit SocketRs(int sd, NetworkManagerRs &nm);

    void set_local(IpAddr addr, uint16_t port, State state);

    bool get_next_data(const uchar **data, size_t *size, IpAddr *src_addr, uint16_t *src_port);
    void ack_data(size_t size);

    void process_message(const NetEventChannelRs::SocketControlMessage &message,
                         NetEventChannelRs::Event &event);

    virtual void handle_data(NetEventChannelRs::DataMessage const &msg, NetEventChannelRs::Event &event);
    void handle_connected(NetEventChannelRs::ConnectedMessage const &msg);
    void handle_closed(NetEventChannelRs::ClosedMessage const &msg);

    NORETURN void inv_state();
    NORETURN void or_closed(Errors::Code err);

    void do_abort(bool remove);

    // Socket descriptor on the server
    int32_t _sd;
    State _state;
    Errors::Code _close_cause;
    bool _blocking;

    IpAddr _local_addr;
    uint16_t _local_port;
    IpAddr _remote_addr;
    uint16_t _remote_port;

    // Reference to the network manager
    NetworkManagerRs &_nm;

    DataQueueRs _recv_queue;
};

}
