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

#include <m3/WorkLoop.h>

#include <sys/socket.h>
#include <sys/types.h>
#include <sys/un.h>

#include "../driver.h"

namespace net {

class FifoDev : public NetDriver {
    class FifoWorkItem : public m3::WorkItem {
    public:
        explicit FifoWorkItem(FifoDev *dev)
            : _dev(dev) {
        }

        void work() override;

    private:
        FifoDev *_dev;
    };

public:
    explicit FifoDev(const char *name, m3::WorkLoop *wl, alloc_cb_func allocCallback,
                     next_buf_cb_func nextBufCallback, recv_cb_func recvCallback);
    ~FifoDev();

    void stop() override;
    bool send(const void *packet, size_t size) override;

    m3::net::MAC readMAC() override;

    bool linkStateChanged() override;
    bool linkIsUp() override;

private:
    int _in_fd;
    int _out_fd;
    sockaddr_un _out_sock;
    alloc_cb_func _allocCallback;
    next_buf_cb_func _nextBufCallback;
    recv_cb_func _recvCallback;
    bool _linkStateChanged;
    FifoWorkItem *_workitem;
};

}
