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

#include <base/log/Lib.h>

#include <m3/net/Socket.h>
#include <m3/net/RawSocket.h>
#include <m3/net/TcpSocket.h>
#include <m3/net/UdpSocket.h>
#include <m3/session/NetworkManager.h>
#include <m3/Exception.h>

#include <thread/ThreadManager.h>

namespace m3 {

Socket* Socket::new_socket(SocketType type, int sd, NetworkManager& nm) {
    switch(type) {
        case SOCK_STREAM:
            return new TcpSocket(sd, nm);
        case SOCK_DGRAM:
            return new UdpSocket(sd, nm);
        case SOCK_RAW:
            return new RawSocket(sd, nm);
        default:
            LLOG(NET, "Socket::new_socket(): Invalid socket type: " << type);
            return nullptr;
    }
}

Socket::Socket(int sd, NetworkManager &nm)
    : m3::TreapNode<Socket, int>(sd),
      _sd(sd),
      _state(None),
      _close_cause(Errors::NONE),
      _local_port(0),
      _remote_port(0),
      _nm(nm),
      _channel(nullptr),
      _blocking(false),
      _wait_event(INVALID_EVENT),
      _waiting(0) {
}

Socket::~Socket() {
    if(_state != Closed || _close_cause != Errors::SOCKET_CLOSED) {
        try {
            close();
        }
        catch(...) {
            // ignore
        }
    }

    // TODO: Notify waiting threads (events and credits)

    // Clear receive queue before potentially destroying the channel,
    // because the queue contains events that point to the channel.
    _recv_queue.clear();

    _nm._sockets.remove(this);
}

void Socket::bind(IpAddr addr, uint16_t port) {
    if(_state != None)
        return inv_state();

    _nm.bind(sd(), addr, port);
    _state = Bound;
    _local_addr = addr;
    _local_port = port;
}

void Socket::listen() {
    throw Exception(Errors::NOT_SUP);
}

void Socket::connect(IpAddr, uint16_t) {
    throw Exception(Errors::NOT_SUP);
}

bool Socket::accept(Socket*&) {
    throw Exception(Errors::NOT_SUP);
}

void Socket::close() {
    // TODO catch exception here?
    _nm.close(sd());
    _state = Closed;
    _close_cause = Errors::SOCKET_CLOSED;
}

ssize_t Socket::send(const void *src, size_t amount) {
    return sendto(src, amount, IpAddr(), 0);
}

ssize_t Socket::recv(void* dst, size_t amount) {
    return recvmsg(dst, amount, nullptr, nullptr);
}

void Socket::process_message(const NetEventChannel::SocketControlMessage & message, NetEventChannel::Event &event) {
    // Notify waiting threads
    if(_waiting > 0) {
        ThreadManager::get().notify(get_wait_event());
        _waiting = 0;
    }

    switch(message.type) {
        case NetEventChannel::DataTransfer:
            return handle_data_transfer(static_cast<NetEventChannel::DataTransferMessage const &>(message));
        case NetEventChannel::AckDataTransfer:
            return handle_ack_data_transfer(static_cast<NetEventChannel::AckDataTransferMessage const &>(message));
        case NetEventChannel::InbandDataTransfer:
            return handle_inband_data_transfer(static_cast<NetEventChannel::InbandDataTransferMessage const &>(message), event);
        case NetEventChannel::SocketAccept:
            return handle_socket_accept(static_cast<NetEventChannel::SocketAcceptMessage const &>(message));
        case NetEventChannel::SocketConnected:
            return handle_socket_connected(static_cast<NetEventChannel::SocketConnectedMessage const &>(message));
        case NetEventChannel::SocketClosed:
            return handle_socket_closed(static_cast<NetEventChannel::SocketClosedMessage const &>(message));
        default:
            throw Exception(Errors::NOT_SUP);
    }
}

void Socket::inv_state() {
    or_closed(Errors::INV_STATE);
}

void Socket::or_closed(Errors::Code err) {
    if(_state == Closed)
        err = _close_cause != Errors::NONE ? _close_cause : Errors::SOCKET_CLOSED;
    throw Exception(err);
}

void Socket::handle_data_transfer(NetEventChannel::DataTransferMessage const &) {
    throw Exception(Errors::NOT_SUP);
}

void Socket::handle_ack_data_transfer(NetEventChannel::AckDataTransferMessage const &) {
    throw Exception(Errors::NOT_SUP);
}

void Socket::handle_inband_data_transfer(NetEventChannel::InbandDataTransferMessage const & msg, NetEventChannel::Event &event) {
    _recv_queue.append(new DataQueue::Item(&msg, std::move(event)));
}

void Socket::handle_socket_accept(NetEventChannel::SocketAcceptMessage const &) {
    throw Exception(Errors::NOT_SUP);
}

void Socket::handle_socket_connected(NetEventChannel::SocketConnectedMessage const &) {
    _state = Connected;
}

void Socket::handle_socket_closed(NetEventChannel::SocketClosedMessage const &msg) {
    _state = Closed;
    _close_cause = msg.cause;
}

bool Socket::get_next_data(const uchar *&data, size_t &size) {
    if(!_recv_queue.get_next_data(data, size))
        fetch_events();

    if(!_recv_queue.get_next_data(data, size)) {
        if(!_blocking) {
            if(_state == Closed)
                inv_state();
            return false;
        }

        do {
            if(_state == Closed)
                inv_state();

            wait_for_event();
        }
        while(!_recv_queue.get_next_data(data, size));
    }
    return true;
}

void Socket::ack_data(size_t size) {
    _recv_queue.ack_data(size);
}

void Socket::fetch_events() {
    for(int i = 0; i < EVENT_FETCH_BATCH_SIZE; i++) {
        auto event = _channel->recv_message();
        if(!event.is_present())
            break;
        // Stop once we received a message for this socket.
        if(_nm.process_event(event) == this)
            break;
    }
}

void Socket::wait_for_event() {
    event_t ev = get_wait_event();
    if(ev == 0)
        _nm.wait_sync();
    else {
        _nm.listen_channel(*_channel);
        _waiting++;
        LLOG(NET, "Socket " << _sd << " is waiting for event " << ev << ".");
        ThreadManager::get().wait_for(ev);
    }
}

event_t Socket::get_wait_event() {
    if(_wait_event == INVALID_EVENT)
        _wait_event = ThreadManager::get().get_wait_event();
    return _wait_event;
}

void Socket::wait_for_credit() {
    event_t ev = _channel->get_credit_event();
    if(ev == 0)
        _nm.wait_sync();
    else {
        _nm.listen_channel(*_channel);
        _nm.wait_for_credit(*_channel);
        LLOG(NET, "Socket " << _sd << " is waiting for credits " << _channel->get_credit_event() << ".");
        ThreadManager::get().wait_for(_channel->get_credit_event());
    }
}

} // namespace m3
