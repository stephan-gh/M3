/*
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

#include <m3/session/ClientSession.h>
#include <m3/com/SendGate.h>
#include <m3/net/Socket.h>
#include <m3/vfs/GenericFile.h>

namespace m3 {

struct MessageHeader {
    explicit MessageHeader()
        : addr(), port(0), size(0) {
    }
    explicit MessageHeader(IpAddr _addr, uint16_t _port, size_t _size)
        : addr(_addr), port(_port), size(_size) {
    }

    static size_t serialize_length() {
        return ostreamsize<uint32_t, uint16_t, size_t>();
    }

    void serialize(Marshaller &m) {
        m.vput(addr.addr(), port, size);
    }

    void unserialize(Unmarshaller &um) {
        uint32_t _addr;
        um.vpull(_addr, port, size);
        addr.addr(_addr);
    }

    IpAddr addr;
    uint16_t port;
    size_t size;
};

class TcpSocket;
// Maybe RawSocket or something...

class NetworkManager : public ClientSession {
    friend Socket;
    friend TcpSocket;

public:
    enum Operation {
        STAT = GenericFile::STAT,
        SEEK = GenericFile::SEEK,
        NEXT_IN = GenericFile::NEXT_IN,
        NEXT_OUT = GenericFile::NEXT_OUT,
        COMMIT = GenericFile::COMMIT,
        CLOSE = GenericFile::CLOSE,
        CREATE,
        BIND,
        LISTEN,
        CONNECT,
        ACCEPT,
        // SEND, // provided by pipes
        // RECV, // provided by pipes
        COUNT
    };

    explicit NetworkManager(const String &service);
    explicit NetworkManager(capsel_t session, capsel_t metagate);
    ~NetworkManager();

    const SendGate &meta_gate() const {
        return _metagate;
    }

    Socket *create(Socket::SocketType type, uint8_t protocol = 0);
    Errors::Code bind(int sd, IpAddr addr, uint16_t port);
    Errors::Code listen(int sd);
    Errors::Code connect(int sd, IpAddr addr, uint16_t port);
    Errors::Code close(int sd);
    Errors::Code as_file(int sd, int mode, MemGate &mem, size_t memsize, fd_t &fd);

private:
    Errors::Code ensure_channel_established();

    void listen_channel(NetEventChannel & _channel);
    void wait_for_credit(NetEventChannel& _channel);
    void wait_sync();

    Socket * process_event(NetEventChannel::Event & event);
    void process_credit(event_t wait_event, size_t waiting);
    void process_sleep();

    SendGate _metagate;
    m3::Treap<Socket> _sockets;
    size_t _waiting_credit;
    Reference<NetEventChannel> _channel;
};

}
