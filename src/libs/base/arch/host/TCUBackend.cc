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

#include <base/arch/host/TCUBackend.h>
#include <base/log/Lib.h>
#include <base/util/Math.h>
#include <base/TCU.h>
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

TCUBackend::UnixSocket::UnixSocket(const char *name, bool pe)
    : fd(),
      addr() {
    fd = socket(AF_UNIX, SOCK_DGRAM, 0);
    if(fd == -1)
        PANIC("Unable to open socket: " << strerror(errno));
    if(fcntl(fd, F_SETFD, FD_CLOEXEC) == -1)
        PANIC("Setting FD_CLOEXEC failed: " << strerror(errno));

    addr.sun_family = AF_UNIX;
    addr.sun_path[0] = '\0';
    if(pe) {
        snprintf(addr.sun_path + 1, sizeof(addr.sun_path) - 1,
                 "%s/%d-%s", Env::tmp_dir(), (int)env()->pe_id, name);
    }
    else {
        snprintf(addr.sun_path + 1, sizeof(addr.sun_path) - 1,
                 "%s/%s", Env::tmp_dir(), name);
    }
}

void TCUBackend::UnixSocket::bind() {
    if(::bind(fd, (struct sockaddr*)&addr, sizeof(addr)) == -1)
        PANIC("Binding socket failed: " << strerror(errno));
}

TCUBackend::TCUBackend()
    : _sock(socket(AF_UNIX, SOCK_DGRAM, 0)),
      _cmd_sock("cmd", true),
      _ack_sock("ack", true),
      _knotify_sock("knotify", false),
      _localsocks(),
      _endpoints() {
    if(_sock == -1)
        PANIC("Unable to open socket: " << strerror(errno));

    _cmd_sock.bind();
    _ack_sock.bind();

    // build socket names for all endpoints on all PEs
    for(peid_t pe = 0; pe < PE_COUNT; ++pe) {
        for(epid_t ep = 0; ep < TOTAL_EPS; ++ep) {
            sockaddr_un *addr = _endpoints + pe * TOTAL_EPS + ep;
            addr->sun_family = AF_UNIX;
            // we can't put that in the format string
            addr->sun_path[0] = '\0';
            snprintf(addr->sun_path + 1, sizeof(addr->sun_path) - 1,
                     "%s/ep_%d.%d", Env::tmp_dir(), (int)pe, (int)ep);
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

        sockaddr_un *addr = _endpoints + env()->pe_id * TOTAL_EPS + ep;
        if(bind(_localsocks[ep], (struct sockaddr*)addr, sizeof(*addr)) == -1)
            PANIC("Binding socket for ep " << ep << " failed: " << strerror(errno));
    }
}

void TCUBackend::shutdown() {
    for(epid_t ep = 0; ep < ARRAY_SIZE(_localsocks); ++ep)
        ::shutdown(_localsocks[ep], SHUT_RD);
}

TCUBackend::~TCUBackend() {
    for(epid_t ep = 0; ep < ARRAY_SIZE(_localsocks); ++ep)
        close(_localsocks[ep]);
}

void TCUBackend::bind_knotify() {
    _knotify_sock.bind();
}

void TCUBackend::notify_kernel(pid_t pid, int status) {
    KNotifyData data = {.pid = pid, .status = status};
    _knotify_sock.send(data);
}

bool TCUBackend::receive_knotify(pid_t *pid, int *status) {
    KNotifyData data;
    if(_knotify_sock.receive(data, false)) {
        *pid = data.pid;
        *status = data.status;
        return true;
    }
    return false;
}

static inline void add_fd(fd_set *set, int fd, int *max_fd) {
    FD_SET(fd, set);
    *max_fd = Math::max(*max_fd, fd);
}

void TCUBackend::wait_for_work() {
    int max_fd = 0;
    fd_set fds[2];
    for(size_t i = 0; i < 2; ++i) {
        FD_ZERO(&fds[i]);
        add_fd(&fds[i], _cmd_sock.fd, &max_fd);
        add_fd(&fds[i], _knotify_sock.fd, &max_fd);
        for(epid_t ep = 0; ep < ARRAY_SIZE(_localsocks); ++ep)
            add_fd(&fds[i], _localsocks[ep], &max_fd);
    }

    UNUSED auto res = ::select(100, &fds[0], nullptr, &fds[1], nullptr);
    assert(res != -1);
}

void TCUBackend::send_command() {
    uint8_t val = 0;
    _cmd_sock.send(val);
}

bool TCUBackend::recv_command() {
    uint8_t val = 0;
    return _cmd_sock.receive(val, false);
}

void TCUBackend::send_ack() {
    uint8_t val = 0;
    _ack_sock.send(val);
}

bool TCUBackend::recv_ack() {
    uint8_t val = 0;
    // block until the ACK for the command arrived
    return _ack_sock.receive(val, true);
}

bool TCUBackend::send(peid_t pe, epid_t ep, const TCU::Buffer *buf) {
    int res = sendto(_sock, buf, buf->length + TCU::HEADER_SIZE, 0,
                     (struct sockaddr*)(_endpoints + pe * TOTAL_EPS + ep), sizeof(sockaddr_un));
    if(res == -1) {
        LLOG(TCUERR, "Sending message to EP " << pe << ":" << ep << " failed: " << strerror(errno));
        return false;
    }
    return true;
}

ssize_t TCUBackend::recv(epid_t ep, TCU::Buffer *buf) {
    ssize_t res = recvfrom(_localsocks[ep], buf, sizeof(*buf), MSG_DONTWAIT, nullptr, nullptr);
    if(res <= 0)
        return -1;
    return res;
}

}
