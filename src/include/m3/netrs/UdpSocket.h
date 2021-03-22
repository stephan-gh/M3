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

class UdpSocketRs : public SocketRs {
    friend class SocketRs;

    explicit UdpSocketRs(int sd, NetworkManagerRs &nm);

public:
    static Reference<UdpSocketRs> create(NetworkManagerRs &nm,
                                         const DgramSocketArgs &args = DgramSocketArgs());

    ~UdpSocketRs();

    /**
     * Bind socket to <address> and <port>.
     *
     * @param addr the local address to bind to
     * @param port the local port to bind to
     */
    void bind(IpAddr addr, uint16_t port);
};

}
