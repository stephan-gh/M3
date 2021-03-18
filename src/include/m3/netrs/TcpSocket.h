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

class TcpSocketRs : public SocketRs {
    friend class SocketRs;

    explicit TcpSocketRs(int sd, NetworkManagerRs &nm);

public:
    static Reference<TcpSocketRs> create(NetworkManagerRs &nm);

    ~TcpSocketRs();

    /**
     * Set socket into listen mode on given address and port.
     */
    void listen(IpAddr local_addr, uint16_t local_port);

    /**
     * Connect the socket to the socket at <addr>:<port>.
     *
     * @param remote_addr address of the socket to connect to
     * @param remote_port port of the socket to connect to
     * @param local_port the local port to bind the socket to
     */
    void connect(IpAddr remote_addr, uint16_t remote_port, uint16_t local_port);

    /**
     * Waits for an incoming connection. The socket needs to be in listening state.
     *
     * @param remote_addr will be set to the remote address
     * @param remote_port will be set to the remote port
     */
    void accept(IpAddr *remote_addr, uint16_t *remote_port);

    /**
     * Sends <amount> bytes from <src> to the connected remote socket.
     *
     * @param src the data to send
     * @param amount the number of bytes to send
     * @return the number of sent bytes (-1 if it would block and the socket is non-blocking)
     */
    ssize_t send(const void *src, size_t amount) {
        return sendto(src, amount, _remote_addr, _remote_port);
    }

    ssize_t sendto(const void *src, size_t amount, IpAddr dst_addr, uint16_t dst_port) override;

    ssize_t recvfrom(void *dst, size_t amount, IpAddr *src_addr, uint16_t *src_port) override;

    /**
     * Closes the transmit side of the socket.
     *
     * In blocking mode, this method blocks until the socket is closed.
     */
    void close();

private:
    void handle_data(NetEventChannelRs::DataMessage const &msg, NetEventChannelRs::Event &event) override;
};

}
