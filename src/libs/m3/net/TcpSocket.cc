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
#include <m3/Exception.h>

namespace m3 {

TcpSocket::TcpSocket(int sd, NetworkManager& nm)
    : Socket(sd, nm)
{
}

TcpSocket::~TcpSocket() {
}

Socket::SocketType TcpSocket::type() noexcept {
    return SOCK_STREAM;
}

void TcpSocket::listen() {
    if(_state != Bound)
        inv_state();

    _nm.listen(sd());
    _state = Listening;
}

void TcpSocket::connect(IpAddr addr, uint16_t port) {
    fetch_events();
    if(_state == Connected) {
        if(!(_remote_addr == addr && _remote_port == port))
            throw Exception(Errors::IS_CONNECTED);
        return;
    }

    if(_state == Connecting)
        throw Exception(Errors::ALREADY_IN_PROGRESS);

    if(_state != None)
        inv_state();

    _nm.connect(sd(), addr, port);
    _remote_addr = addr;
    _remote_port = port;
    _state = Connecting;

    if(!_blocking)
        throw Exception(Errors::IN_PROGRESS);

    // Wait until socket is connected.
    while(_state == Connecting)
        wait_for_event();

    if(_state != Connected)
        inv_state();
}

bool TcpSocket::accept(Socket*& socket) {
    if(_state != Listening)
        inv_state();

    fetch_events();
    if(!_accept_queue.length()) {
        if(!_blocking)
            return false;

        // Block until a new socket was accepted
        while(!_accept_queue.length()) {
            wait_for_event();

            if(_state != Listening)
                inv_state();
        }
    }

    socket = _accept_queue.remove_first();
    return true;
}

ssize_t TcpSocket::sendto(const void *src, size_t amount, IpAddr, uint16_t) {
    if(_state != Connected)
        or_closed(Errors::NOT_CONNECTED);

    do {
        bool success = _channel->inband_data_transfer(_sd, amount, [&](uchar * buf) {
            memcpy(buf, src, amount);
        });

        if(success)
            return static_cast<ssize_t>(amount);
        if(!_blocking)
            return -1;

        // Block until channel regains credits.
        wait_for_credit();
    } while(_state == Connected);

    inv_state();
    return -1;
}

ssize_t TcpSocket::recvmsg(void *dst, size_t amount, IpAddr *src_addr, uint16_t *src_port) {
    // Allow receiving that arrived before the socket/connection was closed.
    if(_state != Connected && _state != Closed)
        throw Exception(Errors::NOT_CONNECTED);

    const uchar * data = nullptr;
    size_t size = 0;
    if(!get_next_data(data, size))
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

void TcpSocket::handle_socket_accept(NetEventChannel::SocketAcceptMessage const & msg) {
    TcpSocket * new_socket = new TcpSocket(msg.new_sd, _nm);
    new_socket->_state = Connected;
    new_socket->_remote_addr = msg.remote_addr;
    new_socket->_remote_port = msg.remote_port;
    new_socket->_channel = _channel;
    _nm._sockets.insert(new_socket);
    _accept_queue.append(new_socket);
}

}
