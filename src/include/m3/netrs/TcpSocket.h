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

#include <m3/netrs/Socket.h>
#include <m3/session/NetworkManagerRs.h>

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
class TcpSocketRs : public SocketRs {
    friend class SocketRs;

    explicit TcpSocketRs(int sd, capsel_t caps, NetworkManagerRs &nm);

public:
    /**
     * Creates a new TCP sockets with given arguments.
     *
     * By default, the socket is in blocking mode, that is, all functions (connect, send, recv, ...)
     * do not return until the operation is complete. This can be changed via set_blocking.
     */
    static Reference<TcpSocketRs> create(NetworkManagerRs &nm,
                                         const StreamSocketArgs &args = StreamSocketArgs());

    ~TcpSocketRs();

    /**
     * Puts this socket into listen mode on the given port.
     *
     * In listen mode, remote connections can be accepted. See accept. Note that in contrast to
     * conventional TCP/IP stacks, listen is a combination of the traditional bind and listen.
     *
     * Listing on this port requires that the used session has permission for this port. This is
     * controlled with the "ports=..." argument in the session argument of M³'s config files.
     *
     * @param local_port the port to listen on
     */
    void listen(uint16_t local_port);

    /**
     * Connect the socket to the socket at <addr>:<port>.
     *
     * @param remote_addr address of the socket to connect to
     * @param remote_port port of the socket to connect to
     */
    void connect(IpAddr remote_addr, uint16_t remote_port);

    /**
     * Accepts a remote connection on this socket
     *
     * The socket has to be put into listen mode first. Note that in contrast to conventional
     * TCP/IP stacks, accept does not yield a new socket, but uses this socket for the accepted
     * connection. Thus, to support multiple connections to the same port, put multiple sockets in
     * listen mode on this port and call accept on each of them.
     *
     * @param remote_addr if not null, it's set to the IP address of the remote endpoint
     * @param remote_port if not null, it's set to the port of the remote endpoint
     */
    void accept(IpAddr *remote_addr, uint16_t *remote_port);

    /**
     * Receives data from the socket into the given buffer.
     *
     * The socket has to be connected first (either via connect or accept). Note that data can be
     * received after the remote side has closed the socket (state RemoteClosed), but not if this
     * side has been closed.
     *
     * @param dst the buffer to receive into
     * @param amount the maximum number of bytes to receive
     * @return the number of received bytes or -1 if it failed
     */
    ssize_t recv(void *dst, size_t amount);

    /**
     * Sends the given data to this socket
     *
     * The socket has to be connected first (either via connect or accept). Note that data can be
     * received after the remote side has closed the socket (state RemoteClosed), but not if this
     * side has been closed.
     *
     * @param src the data to send
     * @param amount the number of bytes to send
     * @return the number of sent bytes or -1 if it failed
     */
    ssize_t send(const void *src, size_t amount);

    /**
     * Closes the connection
     *
     * In contrast to abort, close properly closes the connection to the remote endpoint by going
     * through the TCP protocol.
     *
     * Note that close is *not* called on drop, but has to be called explicitly to ensure that all
     * data is transmitted to the remote end and the connection is properly closed.
     */
    void close();

private:
    void handle_data(NetEventChannelRs::DataMessage const &msg, NetEventChannelRs::Event &event) override;
};

}
