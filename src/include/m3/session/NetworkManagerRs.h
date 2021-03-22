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

#include <base/col/Treap.h>

#include <m3/com/SendGate.h>
#include <m3/netrs/Net.h>
#include <m3/netrs/NetEventChannel.h>
#include <m3/netrs/Socket.h>
#include <m3/session/ClientSession.h>
#include <m3/vfs/GenericFile.h>

namespace m3 {

class UdpSocketRs;
class TcpSocketRs;

class NetworkManagerRs : public ClientSession {
    friend class SocketRs;
    friend class UdpSocketRs;
    friend class TcpSocketRs;

    enum Operation {
        STAT     = GenericFile::STAT,
        SEEK     = GenericFile::SEEK,
        NEXT_IN  = GenericFile::NEXT_IN,
        NEXT_OUT = GenericFile::NEXT_OUT,
        COMMIT   = GenericFile::COMMIT,
        CLOSE    = GenericFile::CLOSE,
        CREATE   = 6,
        BIND,
        LISTEN,
        CONNECT,
        ABORT,
    };

public:
    explicit NetworkManagerRs(const String &service);

private:
    const SendGate &meta_gate() const noexcept {
        return _metagate;
    }

    int32_t create(SocketType type, uint8_t protocol, const SocketArgs &args);
    void add_socket(SocketRs *socket);
    void remove_socket(SocketRs *socket);

    void bind(int32_t sd, IpAddr addr, uint16_t port);
    void listen(int32_t sd, IpAddr local_addr, uint16_t port);
    void connect(int32_t sd, IpAddr remote_addr, uint16_t remote_port, uint16_t local_port);
    bool close(int32_t sd);
    void abort(int32_t sd, bool remove);

    ssize_t send(int32_t sd, IpAddr dst_addr, uint16_t dst_port, const void *data, size_t data_length);

    void as_file(int32_t sd, int mode, MemGate &mem, size_t memsize, fd_t &fd);

    void wait_sync();

    NetEventChannelRs::Event recv_event();
    SocketRs *process_event(NetEventChannelRs::Event &event);

    SendGate _metagate;
    NetEventChannelRs _channel;
    m3::Treap<SocketRs> _sockets;
};

}
