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

#include <m3/net/Socket.h>
#include <m3/session/NetworkManager.h>

namespace m3 {

/**
 * Configures the sizes of the receive and send buffers.
 */
class DgramSocketArgs : public SocketArgs {
public:
    explicit DgramSocketArgs() noexcept : SocketArgs()
    {}

    /**
     * Sets the number of slots and the size in bytes of the receive buffer
     *
     * @param slots the number of slots
     * @param size the total size of the buffer in bytes
     */
    DgramSocketArgs &recv_buffer(size_t slots, size_t size) noexcept {
        rbuf_slots = slots;
        rbuf_size = size;
        return *this;
    }

    /**
     * Sets the number of slots and the size in bytes of the send buffer
     *
     * @param slots the number of slots
     * @param size the total size of the buffer in bytes
     */
    DgramSocketArgs &send_buffer(size_t slots, size_t size) noexcept {
        sbuf_slots = slots;
        sbuf_size = size;
        return *this;
    }
};

/**
 * Represents a datagram socket using the user datagram protocol (UDP)
 */
class UdpSocket : public Socket {
    friend class Socket;

    explicit UdpSocket(int sd, capsel_t caps, NetworkManager &nm);

public:
    /**
     * Creates a new UDP sockets with given arguments.
     *
     * By default, the socket is in blocking mode, that is, all functions (send_to, recv_from, ...)
     * do not return until the operation is complete. This can be changed via set_blocking.
     *
     * @param nm the network manager
     * @param args optionally additional arguments that define the buffer sizes
     */
    static Reference<UdpSocket> create(NetworkManager &nm,
                                         const DgramSocketArgs &args = DgramSocketArgs());

    ~UdpSocket();

    /**
     * @return the local endpoint (only valid if the socket has been bound via bind)
     */
    const Endpoint &local_endpoint() const noexcept {
        return _local_ep;
    }

    /**
     * Binds this socket to the given local port.
     *
     * When bound, packets can be received from remote endpoints.
     *
     * Binding requires that the used session has permission for this port. This is controlled with
     * the "ports=..." argument in the session argument of M³'s config files.
     *
     * @param port the local port to bind to
     */
    void bind(port_t port);

    /**
     * Sends at most <amount> bytes from <src> to the socket at <addr>:<port>.
     *
     * @param src the data to send
     * @param amount the number of bytes to send
     * @param dst_ep destination endpoint
     * @return the number of sent bytes (-1 if it would block and the socket is non-blocking)
     */
    ssize_t send_to(const void *src, size_t amount, const Endpoint &dst_ep);

    /**
     * Receives <amount> or a smaller number of bytes into <dst>.
     *
     * @param dst the destination buffer
     * @param amount the number of bytes to receive
     * @param src_ep if not null, the source endpoint is filled in
     * @return the number of received bytes (-1 if it would block and the socket is non-blocking)
     */
    ssize_t recv_from(void *dst, size_t amount, Endpoint *src_ep);
};

}
