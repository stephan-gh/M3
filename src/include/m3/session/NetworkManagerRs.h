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

#include <base/col/SList.h>

#include <m3/com/SendGate.h>
#include <m3/netrs/Net.h>
#include <m3/netrs/NetEventChannel.h>
#include <m3/netrs/Socket.h>
#include <m3/session/ClientSession.h>
#include <m3/vfs/GenericFile.h>

namespace m3 {

class UdpSocketRs;
class TcpSocketRs;

/**
 * Represents a session at the network service, allowing to create and use sockets
 *
 * To exchange events and data with the server, the NetEventChannel is used, which allows to send
 * and receive multiple messages. Events are used to receive connected or closed events from the
 * server and to send close requests to the server. Transmitted and received data is exchanged via
 * the NetEventChannel in both directions.
 */
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
        BIND     = 6,
        LISTEN,
        CONNECT,
        ABORT,
    };

public:
    /**
     * Creates a new instance for `service`
     *
     * @param service the service name
     */
    explicit NetworkManagerRs(const String &service);

    /**
     * Waits until any socket or a specific socket has received an event.
     *
     * If socket is nullptr, the function waits until any socket has received an event. Otherwise,
     * it waits until the socket with this socket descriptor has received an event.
     *
     * Note: this function uses VPE::sleep if no events are present, which suspends the core until
     * the next TCU message arrives. Thus, calling this function can only be done if all work is
     * done.
     */
    void wait_for_events(SocketRs *socket = nullptr);

    /**
     * Waits until any socket or a specific socket can send events to the server.
     *
     * If socket is nullptr, the function waits until any socket can send. Otherwise, it waits until
     * the socket with this socket descriptor can send.
     *
     * Note: this function uses VPE::sleep if no credits are available, which suspends the core
     * until the next TCU message arrives. Thus, calling this function can only be done if all work
     * is done.
     */
    void wait_for_credits(SocketRs *socket = nullptr);

private:
    const SendGate &meta_gate() const noexcept {
        return _metagate;
    }

    int32_t create(SocketType type, uint8_t protocol, const SocketArgs &args, capsel_t *caps);
    void add_socket(SocketRs *socket);
    void remove_socket(SocketRs *socket);

    IpAddr bind(int32_t sd, uint16_t port);
    IpAddr listen(int32_t sd, uint16_t port);
    uint16_t connect(int32_t sd, IpAddr remote_addr, uint16_t remote_port);
    bool close(int32_t sd);
    void abort(int32_t sd, bool remove);

    ssize_t send(int32_t sd, IpAddr dst_addr, uint16_t dst_port, const void *data, size_t data_length);

    SendGate _metagate;
    SList<SocketRs> _sockets;
};

}
