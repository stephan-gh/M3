/*
 * Copyright (C) 2018, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
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

namespace m3 {

class RawSocket : public Socket {
public:
    explicit RawSocket(int sd, NetworkManager &nm);
    virtual ~RawSocket();

    virtual SocketType type() override;

    virtual Errors::Code bind(IpAddr addr, uint16_t port);

    virtual ssize_t sendto(const void *src, size_t amount, IpAddr addr, uint16_t port) override;
    virtual ssize_t recvmsg(void *dst, size_t amount, IpAddr *src_addr, uint16_t *src_port) override;
};

}
