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
#include <m3/net/Net.h>
#include <m3/net/NetEventChannel.h>
#include <m3/net/Socket.h>
#include <m3/session/ClientSession.h>
#include <m3/vfs/GenericFile.h>

namespace m3 {

class UdpSocket;
class TcpSocket;

/**
 * Represents a session at the network service, allowing to create and use sockets
 *
 * To exchange events and data with the server, the NetEventChannel is used, which allows to send
 * and receive multiple messages. Events are used to receive connected or closed events from the
 * server and to send close requests to the server. Transmitted and received data is exchanged via
 * the NetEventChannel in both directions.
 */
class NetworkManager : public ClientSession {
    friend class Socket;
    friend class UdpSocket;
    friend class TcpSocket;

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
        CREATE,
        GET_SGATE,
        OPEN_FILE,
    };

public:
    /**
     * A bitmask of directions for wait.
     */
    enum Direction {
        // Data can be received or the socket state has changed
        INPUT         = 1,
        // Data can be sent
        OUTPUT        = 2,
    };

    /**
     * Creates a new instance for `service`
     *
     * @param service the service name
     */
    explicit NetworkManager(const String &service);

    /**
     * Waits until any socket has received input (including state-change events) or can produce
     * output.
     *
     * Note that Direction::INPUT has to be specified to process events (state changes and data).
     *
     * Note: this function uses VPE::sleep if tick_sockets returns false, which suspends the core
     * until the next TCU message arrives. Thus, calling this function can only be done if all work
     * is done.
     *
     * @param dirs the directions to check
     */
    void wait(uint dirs = Direction::INPUT | Direction::OUTPUT);

    /**
     * Waits until any socket has received input (including state-change events) or can produce
     * output or the given timeout is reached.
     *
     * Note that Direction::INPUT has to be specified to process events (state changes and data).
     *
     * Note: this function uses VPE::sleep if tick_sockets returns false, which suspends the core
     * until the next TCU message arrives. Thus, calling this function can only be done if all work
     * is done.
     *
     * @param timeout the number of nanoseconds to wait at most
     * @param dirs the directions to check
     */
    void wait_for(uint64_t timeout, uint dirs = Direction::INPUT | Direction::OUTPUT);

private:
    static KIF::CapRngDesc get_sgate(ClientSession &sess);

    const SendGate &meta_gate() const noexcept {
        return _metagate;
    }

    int32_t create(SocketType type, uint8_t protocol, const SocketArgs &args, capsel_t *caps);
    void add_socket(Socket *socket);
    void remove_socket(Socket *socket);

    IpAddr bind(int32_t sd, port_t port);
    IpAddr listen(int32_t sd, port_t port);
    port_t connect(int32_t sd, IpAddr remote_addr, port_t remote_port);
    bool close(int32_t sd);
    void abort(int32_t sd, bool remove);

    bool tick_sockets(uint dirs = Direction::INPUT | Direction::OUTPUT);

    SendGate _metagate;
    SList<Socket> _sockets;
};

}
