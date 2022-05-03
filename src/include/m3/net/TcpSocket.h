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

#include <m3/net/Socket.h>
#include <m3/session/NetworkManager.h>

namespace m3 {

/**
 * Configures the sizes of the receive and send buffers.
 */
class StreamSocketArgs : public SocketArgs {
public:
    explicit StreamSocketArgs() noexcept : SocketArgs() {
        rbuf_slots = 0;
        sbuf_slots = 0;
    }

    /**
     * Sets the size in bytes of the receive buffer
     *
     * @param size the total size of the buffer in bytes
     */
    StreamSocketArgs &recv_buffer(size_t size) noexcept {
        rbuf_size = size;
        return *this;
    }

    /**
     * Sets the size in bytes of the send buffer
     *
     * @param size the total size of the buffer in bytes
     */
    StreamSocketArgs &send_buffer(size_t size) noexcept {
        sbuf_size = size;
        return *this;
    }
};

/**
 * Represents a stream socket using the transmission control protocol (TCP)
 */
class TcpSocket : public Socket {
    friend class Socket;

    explicit TcpSocket(int fd, capsel_t caps, NetworkManager &nm);

public:
    /**
     * Creates a new TCP sockets with given arguments.
     *
     * By default, the socket is in blocking mode, that is, all functions (connect, send, recv, ...)
     * do not return until the operation is complete. This can be changed via set_blocking.
     */
    static FileRef<TcpSocket> create(NetworkManager &nm,
                                     const StreamSocketArgs &args = StreamSocketArgs());

    ~TcpSocket();

    /**
     * Puts this socket into listen mode on the given port.
     *
     * In listen mode, remote connections can be accepted. See accept. Note that in contrast to
     * conventional TCP/IP stacks, listen is a combination of the traditional bind and listen.
     *
     * Listing on this port requires that the used session has permission for this port. This is
     * controlled with the "tcp=..." argument in the session argument of M³'s config files.
     *
     * @param port the port to listen on
     */
    void listen(port_t port);

    virtual bool connect(const Endpoint &endpoint) override;

    /**
     * Accepts a remote connection on this socket
     *
     * The socket has to be put into listen mode first. Note that in contrast to conventional
     * TCP/IP stacks, accept does not yield a new socket, but uses this socket for the accepted
     * connection. Thus, to support multiple connections to the same port, put multiple sockets in
     * listen mode on this port and call accept on each of them.
     *
     * @param remote_ep if not null, it's set to the remote endpoint
     * @return true if the socket is connected (false if the socket is non-blocking and the
     *     connection is in progress)
     */
    bool accept(Endpoint *remote_ep);

    virtual ssize_t send(const void *src, size_t amount) override;

    virtual ssize_t recv(void *dst, size_t amount) override;

    /**
     * Closes the socket.
     *
     * In contrast to abort, close properly closes the connection to the remote endpoint by going
     * through the TCP protocol.
     *
     * Note that close is called in the destructor in case the socket has not be closed/aborted yet.
     *
     * @return Errors::NONE if the socket has been successfully closed or Errors::WOULD_BLOCK if the
     *     close request could not been sent or Errors::IN_PROGRESS if the close request was sent,
     *     but the socket is not closed yet. The former two errors only occur in non-blocking mode.
     */
    Errors::Code close();

    /**
     * Performs a hard abort by closing the socket on our end and dropping all data. Note that
     * submitted packets for sending are not guaranteed to be sent out.
     */
    void abort();

private:
    void handle_data(NetEventChannel::DataMessage const &msg,
                     NetEventChannel::Event &event) override;
    void remove() noexcept override;
};

}
