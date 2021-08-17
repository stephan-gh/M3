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
#include <base/util/Reference.h>

#include <m3/net/DataQueue.h>
#include <m3/net/Net.h>

namespace m3 {

class NetworkManager;

/**
 * Arguments for socket creations that define the buffer sizes
 */
struct SocketArgs {
    explicit SocketArgs()
        : rbuf_slots(4),
          rbuf_size(16 * 1024),
          sbuf_slots(4),
          sbuf_size(16 * 1024)
    {}

    size_t rbuf_slots;
    size_t rbuf_size;
    size_t sbuf_slots;
    size_t sbuf_size;
};

/**
 * The base class of all sockets, which provides the common functionality
 */
class Socket : public SListItem, public RefCounted {
    friend class NetworkManager;

    static const int EVENT_FETCH_BATCH_SIZE = 4;

public:
    /**
     * The states sockets can be in
     */
    enum State {
        // The socket is bound to a local address and port
        Bound,
        // The socket is listening on a local address and port for remote connections
        Listening,
        // The socket is currently connecting to a remote endpoint
        Connecting,
        // The socket is connected to a remote endpoint
        Connected,
        // The remote side has closed the connection
        RemoteClosed,
        // The socket is currently being closed, initiated by our side
        Closing,
        // The socket is closed (default state)
        Closed
    };

    virtual ~Socket();

    /**
     * @return the socket descriptor used to identify this socket within the session on the server
     */
    int sd() const noexcept {
        return _sd;
    }

    /**
     * @return the current state of the socket
     */
    State state() const noexcept {
        return _state;
    }

    /**
     * @return whether the socket is currently in blocking mode
     */
    bool blocking() const noexcept {
        return _blocking;
    }

    /**
     * Sets whether the socket is using blocking mode.
     *
     * In blocking mode, all functions (connect, send_to, recv_from, ...) do not return until the
     * operation is complete. In non-blocking mode, all functions return -1 in case they would need
     * to block, that is, wait until an event is received or further data can be sent.
     *
     * @param blocking whether socket operates in blocking or non-block mode (default = blocking)
     */
    void blocking(bool blocking) noexcept {
        _blocking = blocking;
    }

protected:
    explicit Socket(int sd, capsel_t caps, NetworkManager &nm);

    bool get_next_data(const uchar **data, size_t *size, Endpoint *ep);
    void ack_data(size_t size);

    ssize_t do_send(const void *src, size_t amount, const Endpoint &ep);
    ssize_t do_recv(void *dst, size_t amount, Endpoint *ep);

    void process_message(const NetEventChannel::ControlMessage &message,
                         NetEventChannel::Event &event);

    virtual void handle_data(NetEventChannel::DataMessage const &msg, NetEventChannel::Event &event);
    void handle_connected(NetEventChannel::ConnectedMessage const &msg);
    void handle_close_req(NetEventChannel::CloseReqMessage const &msg);
    void handle_closed(NetEventChannel::ClosedMessage const &msg);

    void tear_down();
    void disconnect();

    void wait_for_events();
    void wait_for_credits();
    bool process_events();
    void fetch_replies();
    bool can_send();

    int32_t _sd;
    State _state;
    bool _blocking;

    Endpoint _local_ep;
    Endpoint _remote_ep;

    NetworkManager &_nm;

    NetEventChannel _channel;
    DataQueue _recv_queue;
};

}
