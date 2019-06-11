/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <m3/net/Net.h>

namespace net {

class NetDriver {
public:
    typedef bool(&alloc_cb_func)(void *&pkt, void *&buf, size_t &bufSize, size_t size);
    typedef void(&next_buf_cb_func)(void *&pkt, void *&buf, size_t &bufSize);
    typedef void(&recv_cb_func)(void *pkt);

    static NetDriver *create(const char *name, m3::WorkLoop *wl, alloc_cb_func allocCallback,
                             next_buf_cb_func nextBufCallback, recv_cb_func recvCallback);

    virtual ~NetDriver() {
    }

    virtual m3::net::MAC readMAC() = 0;

    virtual void stop() = 0;

    virtual bool send(const void *packet, size_t size) = 0;

    virtual bool linkStateChanged() = 0;
    virtual bool linkIsUp() = 0;
};

}
