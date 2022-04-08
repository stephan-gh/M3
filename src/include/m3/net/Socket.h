/*
 * Copyright (C) 2021 Nils Asmussen, Barkhausen Institut
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
 * Copyright (C) 2017, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
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
#include <m3/vfs/File.h>

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
class Socket : public SListItem, public File {
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

    virtual Errors::Code try_stat(FileInfo &) const override {
        return Errors::NOT_SUP;
    }
    virtual size_t seek(size_t, int) override {
        throw Exception(Errors::NOT_SUP);
    }
    virtual void map(Reference<Pager> &, goff_t *, size_t, size_t, int, int) const override {
        throw Exception(Errors::NOT_SUP);
    }

    virtual FileRef<File> clone() const override {
        throw Exception(Errors::NOT_SUP);
    }
    virtual void delegate(ChildActivity &) override {
        throw Exception(Errors::NOT_SUP);
    }
    virtual void serialize(Marshaller &) override {
        throw Exception(Errors::NOT_SUP);
    }

    virtual bool check_events(uint events) override {
        fetch_replies();

        return ((events & File::INPUT) != 0 && process_events()) ||
            ((events & File::OUTPUT) != 0 && can_send());
    }

    virtual char type() const noexcept override {
        return 's';
    }

    /**
     * @return the socket descriptor used on the server side
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
     * Checks whether there is data to receive.
     *
     * Note that this function does not process events. To receive data, any receive function on
     * this socket or [`NetworkManager::wait`] has to be called.
     *
     * @return true if data can currently be received from the socket
     */
    bool has_data() const noexcept {
        return _recv_queue.has_data();
    }

protected:
    explicit Socket(int sd, capsel_t caps, NetworkManager &nm);

    virtual void enable_notifications() override {
        // nothing to do
    }

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

    void tear_down() noexcept;
    void disconnect();

    void wait_for_events();
    void wait_for_credits();
    bool process_events();
    void fetch_replies();
    bool can_send();

    int _sd;
    State _state;

    Endpoint _local_ep;
    Endpoint _remote_ep;

    NetworkManager &_nm;

    NetEventChannel _channel;
    DataQueue _recv_queue;
};

}
