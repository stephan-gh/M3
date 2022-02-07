/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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
#include <base/log/Lib.h>

#include <sys/un.h>
#include <sys/socket.h>
#include <poll.h>
#include <errno.h>

namespace m3 {

class TCUBackend {
public:
    struct UnixSocket {
        explicit UnixSocket(const char *name, bool tile);

        void bind();

        template<typename T>
        void send(const T &data) {
            int res = sendto(fd, &data, sizeof(data), 0, (struct sockaddr*)(&addr), sizeof(addr));
            if(res == -1)
                LLOG(TCUERR, "send failed: " << strerror(errno));
        }

        template<typename T>
        bool receive(T &data, bool block) {
            return recvfrom(fd, &data, sizeof(data), block ? 0 : MSG_DONTWAIT, nullptr, nullptr) > 0;
        }

        int fd;
        sockaddr_un addr;
    };

    struct KNotifyData {
        pid_t pid;
        int status;
    } PACKED;

    explicit TCUBackend();
    ~TCUBackend();

    void shutdown();

    bool send(tileid_t tile, epid_t ep, const TCU::Buffer *buf);
    ssize_t recv(epid_t ep, TCU::Buffer *buf);

    void bind_knotify();
    void notify_kernel(pid_t pid, int status);
    bool receive_knotify(pid_t *pid, int *status);

    void wait_for_work(uint64_t timeout);

    void send_command();
    bool recv_command();
    void send_ack();
    bool recv_ack();

private:
    int _sock;
    UnixSocket _cmd_sock;
    UnixSocket _ack_sock;
    UnixSocket _knotify_sock;
    int _localsocks[TOTAL_EPS];
    sockaddr_un _endpoints[TILE_COUNT * TOTAL_EPS];
};

}
