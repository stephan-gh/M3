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

#include <base/arch/host/DTUBackend.h>
#include <base/log/Lib.h>
#include <base/DTU.h>
#include <base/Panic.h>

#include <sys/types.h>
#include <sys/ipc.h>
#include <sys/msg.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <unistd.h>

namespace m3 {

DTUBackend::DTUBackend()
    : _sock(socket(AF_UNIX, SOCK_DGRAM, 0)),
      _localsocks(),
      _endpoints() {
    if(_sock == -1)
        PANIC("Unable to open socket: " << strerror(errno));

    // build socket names for all endpoints on all PEs
    for(peid_t pe = 0; pe < PE_COUNT; ++pe) {
        for(epid_t ep = 0; ep < EP_COUNT; ++ep) {
            sockaddr_un *addr = _endpoints + pe * EP_COUNT + ep;
            addr->sun_family = AF_UNIX;
            // we can't put that in the format string
            addr->sun_path[0] = '\0';
            snprintf(addr->sun_path + 1, sizeof(addr->sun_path) - 1, "m3_ep_%d.%d", (int)pe, (int)ep);
        }
    }

    // create sockets and bind them for our own endpoints
    for(epid_t ep = 0; ep < ARRAY_SIZE(_localsocks); ++ep) {
        _localsocks[ep] = socket(AF_UNIX, SOCK_DGRAM, 0);
        if(_localsocks[ep] == -1)
            PANIC("Unable to create socket for ep " << ep << ": " << strerror(errno));

        // if we do fork+exec in kernel/lib we want to close all sockets. they are recreated anyway
        if(fcntl(_localsocks[ep], F_SETFD, FD_CLOEXEC) == -1)
            PANIC("Setting FD_CLOEXEC failed: " << strerror(errno));

        sockaddr_un *addr = _endpoints + env()->pe * EP_COUNT + ep;
        if(bind(_localsocks[ep], (struct sockaddr*)addr, sizeof(*addr)) == -1)
            PANIC("Binding socket for ep " << ep << " failed: " << strerror(errno));
    }
}

void DTUBackend::shutdown() {
    for(epid_t ep = 0; ep < ARRAY_SIZE(_localsocks); ++ep)
        ::shutdown(_localsocks[ep], SHUT_RD);
}

DTUBackend::~DTUBackend() {
    for(epid_t ep = 0; ep < ARRAY_SIZE(_localsocks); ++ep)
        close(_localsocks[ep]);
}

bool DTUBackend::send(peid_t pe, epid_t ep, const DTU::Buffer *buf) {
    int res = sendto(_sock, buf, buf->length + DTU::HEADER_SIZE, 0,
                     (struct sockaddr*)(_endpoints + pe * EP_COUNT + ep), sizeof(sockaddr_un));
    if(res == -1) {
        LLOG(DTUERR, "Sending message to EP " << pe << ":" << ep << " failed: " << strerror(errno));
        return false;
    }
    return true;
}

ssize_t DTUBackend::recv(epid_t ep, DTU::Buffer *buf) {
    ssize_t res = recvfrom(_localsocks[ep], buf, sizeof(*buf), MSG_DONTWAIT, nullptr, nullptr);
    if(res <= 0)
        return -1;
    return res;
}

}
