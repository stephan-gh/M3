/*
 * Copyright (C) 2016-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/Config.h>
#include <base/TCU.h>

#include <sys/un.h>
#include <poll.h>

namespace m3 {

class TCUBackend {
public:
    struct KNotifyData {
        pid_t pid;
        int status;
    } PACKED;

    explicit TCUBackend();
    ~TCUBackend();

    void shutdown();

    bool send(peid_t pe, epid_t ep, const TCU::Buffer *buf);
    ssize_t recv(epid_t ep, TCU::Buffer *buf);

    void bind_knotify();
    void notify_kernel(pid_t pid, int status);
    bool receive_knotify(pid_t *pid, int *status);

private:
    int _sock;
    int _knotify_sock;
    sockaddr_un _knotify_addr;
    int _localsocks[EP_COUNT];
    sockaddr_un _endpoints[PE_COUNT * EP_COUNT];
};

}
