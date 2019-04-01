/*
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

#include <m3/net/TcpSocket.h>
#include <m3/session/NetworkManager.h>

namespace m3 {

TcpSocket::TcpSocket(int sd, NetworkManager& nm)
    : Socket(sd, nm)
{
}

TcpSocket::~TcpSocket() {
}

Socket::SocketType TcpSocket::type() {
    return SOCK_STREAM;
}

Errors::Code TcpSocket::listen() {
    if(_state != Bound)
        return inv_state();

    return update_status(_nm.listen(sd()), Listening);
}

Errors::Code TcpSocket::connect(IpAddr addr, uint16_t port) {
    fetch_events();
    if(_state == Connected)
        return _remote_addr == addr && _remote_port == port ? Errors::NONE : Errors::IS_CONNECTED;

    if(_state == Connecting)
        return Errors::ALREADY_IN_PROGRESS;

    if(_state != None)
        return inv_state();

    auto result = _nm.connect(sd(), addr, port);
    if(result == Errors::NONE) {
        _remote_addr = addr;
        _remote_port = port;
        _state = Connecting;

        if(!_blocking)
            return Errors::IN_PROGRESS;

        // Wait until socket is connected.
        while(_state == Connecting) {
            wait_for_event();
        }
        return _state == Connected ? Errors::NONE : inv_state();
    } else
        return result;
}

Errors::Code TcpSocket::accept(Socket*& socket) {
    if(_state != Listening)
        return inv_state();

    fetch_events();
    if(!_accept_queue.length()) {
        if(!_blocking)
            return Errors::WOULD_BLOCK;

        // Block until a new socket was accepted
        while(!_accept_queue.length()) {
            wait_for_event();

            if(_state != Listening)
                return inv_state();
        }
    }

    socket = _accept_queue.remove_first();
    return Errors::NONE;
}

ssize_t TcpSocket::sendto(const void *src, size_t amount, IpAddr, uint16_t) {
    if(_state != Connected) {
        Errors::last = or_closed(Errors::NOT_CONNECTED);
        return -1;
    }

    do {
        auto err = _channel->inband_data_transfer(_sd, amount, [&](uchar * buf) {
            memcpy(buf, src, amount);
        });

        if(err == Errors::NONE)
            return static_cast<ssize_t>(amount);

        if(err != Errors::MISS_CREDITS || !_blocking)
        {
            Errors::last = err;
            return -1;
        }

        // Block until channel regains credits.
        wait_for_credit();
    } while(_state == Connected);

    Errors::last = inv_state();
    return -1;
}

ssize_t TcpSocket::recvmsg(void *dst, size_t amount, IpAddr *src_addr, uint16_t *src_port) {
    // Allow receiving that arrived before the socket/connection was closed.
    if(_state != Connected && _state != Closed) {
        Errors::last = Errors::NOT_CONNECTED;
        return -1;
    }

    const uchar * data = nullptr;
    size_t size = 0;
    Errors::last = get_next_data(data, size);
    if(Errors::last != Errors::NONE)
        return -1;

    if(src_addr)
        *src_addr = _remote_addr;
    if(src_port)
        *src_port = _remote_port;

    size_t recv_size = Math::min(size, amount);
    memcpy(dst, data, recv_size);

    _recv_queue.ack_data(recv_size);

    return static_cast<ssize_t>(recv_size);
}

Errors::Code TcpSocket::handle_socket_accept(NetEventChannel::SocketAcceptMessage const & msg) {
    TcpSocket * new_socket = new TcpSocket(msg.new_sd, _nm);
    new_socket->_state = Connected;
    new_socket->_remote_addr = msg.remote_addr;
    new_socket->_remote_port = msg.remote_port;
    new_socket->_channel = _channel;
    _nm._sockets.insert(new_socket);
    _accept_queue.append(new_socket);
    return Errors::NONE;
}

}
