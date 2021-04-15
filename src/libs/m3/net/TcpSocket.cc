/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
 * Copyright (C) 2018, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
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

#include <m3/Exception.h>
#include <m3/net/Socket.h>
#include <m3/net/TcpSocket.h>
#include <m3/session/NetworkManager.h>

namespace m3 {

TcpSocket::TcpSocket(int sd, capsel_t caps, NetworkManager &nm)
    : Socket(sd, caps, nm) {
}

TcpSocket::~TcpSocket() {
    try {
        do_abort(true);
    }
    catch(...) {
        // ignore errors here
    }

    _nm.remove_socket(this);
}

Reference<TcpSocket> TcpSocket::create(NetworkManager &nm, const StreamSocketArgs &args) {
    capsel_t caps;
    int sd = nm.create(SocketType::STREAM, 0, args, &caps);
    auto sock = new TcpSocket(sd, caps, nm);
    nm.add_socket(sock);
    return Reference<TcpSocket>(sock);
}

void TcpSocket::close() {
    // ensure that we don't receive more data (which could block our event channel and thus prevent
    // us from receiving the closed event)
    _state = State::Closing;
    _recv_queue.clear();

    // send the close request; this has to be blocking
    while(!_channel.send_close_req())
        wait_for_credits();

    // now wait for the response; can be non-blocking
    while(_state != State::Closed) {
        if(!_blocking)
            throw Exception(Errors::IN_PROGRESS);

        wait_for_events();
    }
}

void TcpSocket::listen(port_t local_port) {
    if(_state != State::Closed)
        throw Exception(Errors::INV_STATE);

    IpAddr local_addr = _nm.listen(sd(), local_port);
    set_local(local_addr, local_port, State::Listening);
}

void TcpSocket::connect(IpAddr remote_addr, port_t remote_port) {
    if(_state == State::Connected) {
        if(!(_remote_addr == remote_addr && _remote_port == remote_port))
            throw Exception(Errors::IS_CONNECTED);
        return;
    }

    if(_state == State::Connecting)
        throw Exception(Errors::ALREADY_IN_PROGRESS);

    port_t local_port = _nm.connect(sd(), remote_addr, remote_port);
    _state = State::Connecting;
    _remote_addr = remote_addr;
    _remote_port = remote_port;
    _local_port = local_port;

    if(!_blocking)
        throw Exception(Errors::IN_PROGRESS);

    while(_state == State::Connecting)
        wait_for_events();

    if(_state != Connected)
        throw Exception(Errors::CONNECTION_FAILED);
}

void TcpSocket::accept(IpAddr *remote_addr, port_t *remote_port) {
    if(_state == State::Connected) {
        if(remote_addr)
            *remote_addr = _remote_addr;
        if(remote_port)
            *remote_port = _remote_port;
        return;
    }
    if(_state == State::Connecting)
        throw Exception(Errors::ALREADY_IN_PROGRESS);
    if(_state != State::Listening)
        throw Exception(Errors::INV_STATE);

    _state = State::Connecting;
    while(_state == State::Connecting)
        wait_for_events();

    if(_state != State::Connected)
        throw Exception(Errors::CONNECTION_FAILED);

    if(remote_addr)
        *remote_addr = _remote_addr;
    if(remote_port)
        *remote_port = _remote_port;
}

ssize_t TcpSocket::recv(void *dst, size_t amount) {
    // receive is possible with an established connection or a connection that that has already been
    // closed by the remote side
    if(_state != Connected && _state != RemoteClosed)
        throw Exception(Errors::NOT_CONNECTED);

    return Socket::do_recv(dst, amount, nullptr, nullptr);
}

ssize_t TcpSocket::send(const void *src, size_t amount) {
    // like for receive: still allow sending if the remote side closed the connection
    if(_state != Connected && _state != RemoteClosed)
        throw Exception(Errors::NOT_CONNECTED);

    return Socket::do_send(src, amount, _remote_addr, _remote_port);
}

void TcpSocket::handle_data(NetEventChannel::DataMessage const &msg, NetEventChannel::Event &event) {
    if(_state != Closed && _state != Closing)
        Socket::handle_data(msg, event);
}

}
