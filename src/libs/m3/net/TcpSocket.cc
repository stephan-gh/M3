/*
 * Copyright (C) 2021 Nils Asmussen, Barkhausen Institut
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
 * Copyright (C) 2018, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
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

TcpSocket::TcpSocket(int fd, capsel_t caps, NetworkManager &nm)
    : Socket(fd, caps, nm) {
}

TcpSocket::~TcpSocket() {
    remove();
}

Reference<TcpSocket> TcpSocket::create(NetworkManager &nm, const StreamSocketArgs &args) {
    capsel_t caps;
    int fd = nm.create(SocketType::STREAM, 0, args, &caps);
    auto sock = new TcpSocket(fd, caps, nm);
    nm.add_socket(sock);
    return Reference<TcpSocket>(sock);
}

void TcpSocket::listen(port_t port) {
    if(_state != State::Closed)
        throw Exception(Errors::INV_STATE);

    IpAddr addr = _nm.listen(fd(), port);
    _local_ep.addr = addr;
    _local_ep.port = port;
    _state = State::Listening;
}

bool TcpSocket::connect(const Endpoint &endpoint) {
    if(_state == State::Connected) {
        if(_remote_ep != endpoint)
            throw Exception(Errors::IS_CONNECTED);
        return true;
    }

    if(_state == State::Connecting)
        throw Exception(Errors::ALREADY_IN_PROGRESS);

    Endpoint local_ep = _nm.connect(fd(), endpoint);
    _state = State::Connecting;
    _remote_ep = endpoint;
    _local_ep = local_ep;

    if(!_blocking)
        return false;

    while(_state == State::Connecting)
        wait_for_events();

    if(_state != Connected)
        throw Exception(Errors::CONNECTION_FAILED);
    return true;
}

bool TcpSocket::accept(Endpoint *remote_ep) {
    if(_state == State::Connected) {
        if(remote_ep)
            *remote_ep = _remote_ep;
        return true;
    }
    if(_state == State::Connecting)
        throw Exception(Errors::ALREADY_IN_PROGRESS);
    if(_state != State::Listening)
        throw Exception(Errors::INV_STATE);

    _state = State::Connecting;
    while(_state == State::Connecting) {
        if(!is_blocking())
            return false;
        wait_for_events();
    }

    if(_state != State::Connected)
        throw Exception(Errors::CONNECTION_FAILED);

    if(remote_ep)
        *remote_ep = _remote_ep;
    return true;
}

ssize_t TcpSocket::recv(void *dst, size_t amount) {
    // receive is possible with an established connection or a connection that that has already been
    // closed by the remote side
    if(_state != Connected && _state != RemoteClosed)
        throw Exception(Errors::NOT_CONNECTED);

    return Socket::do_recv(dst, amount, nullptr);
}

ssize_t TcpSocket::send(const void *src, size_t amount) {
    // like for receive: still allow sending if the remote side closed the connection
    if(_state != Connected && _state != RemoteClosed)
        throw Exception(Errors::NOT_CONNECTED);

    const uint8_t *src_bytes = reinterpret_cast<const uint8_t*>(src);
    ssize_t total = 0;
    while(amount > 0) {
        size_t now = Math::min(amount, NetEventChannel::MAX_PACKET_SIZE);
        ssize_t sent = Socket::do_send(src_bytes, now, _remote_ep);
        if(sent == -1 && total == 0)
            return -1;
        if(sent == -1)
            return total;

        total += sent;
        amount -= static_cast<size_t>(sent);
        src_bytes += sent;
    }
    return total;
}

void TcpSocket::handle_data(NetEventChannel::DataMessage const &msg, NetEventChannel::Event &event) {
    if(_state != Closed && _state != Closing)
        Socket::handle_data(msg, event);
}

Errors::Code TcpSocket::close() {
    if(_state == State::Closed)
        return Errors::NONE;

    if(_state == State::Closing)
        throw Exception(Errors::ALREADY_IN_PROGRESS);

    // send the close request; this has to be blocking
    while(!_channel.send_close_req()) {
        if(!is_blocking())
            return Errors::WOULD_BLOCK;

        wait_for_credits();
    }

    // ensure that we don't receive more data (which could block our event channel and thus
    // prevent us from receiving the closed event)
    _state = State::Closing;
    _recv_queue.clear();

    // now wait for the response; can be non-blocking
    while(_state != State::Closed) {
        if(!is_blocking())
            return Errors::IN_PROGRESS;

        wait_for_events();
    }
    return Errors::NONE;
}

void TcpSocket::abort() {
    if(_state == State::Closed)
        return;

    _nm.abort(fd(), false);
    _recv_queue.clear();
    disconnect();
}

void TcpSocket::remove() noexcept {
    // use blocking mode here, because we cannot leave the destructor until the socket is closed.
    set_blocking(true);

    try {
        close();
    }
    catch(...) {
        // ignore errors
    }
}

}
