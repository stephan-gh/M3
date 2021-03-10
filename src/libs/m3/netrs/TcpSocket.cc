/*
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
#include <m3/netrs/Socket.h>
#include <m3/netrs/TcpSocket.h>
#include <m3/session/NetworkManagerRs.h>

namespace m3 {

TcpSocketRs::TcpSocketRs(NetworkManagerRs &nm)
    : _blocking(), _is_closed(1), _socket(SocketType::SOCK_STREAM, nm, 0) {
}

TcpSocketRs::~TcpSocketRs() {
    //Try close anyways. Maybe it wasnt closed yet
    if(!_is_closed) {
        close();
    }
}

void TcpSocketRs::set_blocking(bool should_block) {
    _blocking = should_block;
}

void TcpSocketRs::listen(IpAddr addr, uint16_t port) {
    _socket._nm.listen(_socket._sd, addr, port);
    if(_blocking) {
        wait_for_state(TcpState::Listen);
    }
    _is_closed = false;
}

void TcpSocketRs::connect(IpAddr remote_addr, uint16_t remote_port, IpAddr local_addr, uint16_t local_port) {
    _socket._nm.connect(_socket._sd, remote_addr, remote_port, local_addr, local_port);
    if(_blocking) {
        wait_for_state(TcpState::Established);
    }
    _is_closed = false;
}

///When non blocking always returns a package, but it might be empty.
///When blocking, blocks until a non-empty package is received.
m3::net::NetData TcpSocketRs::recv() {
    if(_blocking) {
        while(1) {
            m3::net::NetData pkg = _socket._nm.recv(_socket._sd);
            if(!pkg.is_empty()) {
                return pkg;
            }
            //else keep waiting.
            //TODO should yield?
        }
    }
    else {
        return _socket._nm.recv(_socket._sd);
    }
}

void TcpSocketRs::send(uint8_t *data, uint32_t size) {
    //Note on tcp the we let the service do ip handling, since the socket must be connected
    //before use. Therefore all ips are unspecified
    _socket._nm.send(_socket._sd, IpAddr(), 0, IpAddr(), 0, data, size);
}

void TcpSocketRs::close() {
    _is_closed = true;
    _socket._nm.close(_socket._sd);
    /* Do not wait, otherwise an execption could occure if we query state on closed socket.
	if (_blocking){
	    wait_for_state(TcpState::Closed);
	}
	*/
}

///Returns the tcp state, or TcpState::Invalid, if tcp state was queried on a non TCP socket.
TcpState TcpSocketRs::state() {
    SocketState state = _socket._nm.get_state(_socket._sd);
    return state.tcp_state();
}

void TcpSocketRs::wait_for_state(TcpState target_state) {
    while(state() != target_state) {
        //TODO check for error
        //TODO signal yield?
    }
}

}
