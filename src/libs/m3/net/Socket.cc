/*
 * Copyright (C) 2021 Nils Asmussen, Barkhausen Institut
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
 * Copyright (C) 2017, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
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

#include <m3/net/Net.h>
#include <m3/net/Socket.h>
#include <m3/net/TcpSocket.h>
#include <m3/net/UdpSocket.h>
#include <m3/session/NetworkManager.h>

namespace m3 {

Socket::Socket(int sd, capsel_t caps, NetworkManager &nm)
    : SListItem(),
      RefCounted(),
      _sd(sd),
      _state(Closed),
      _blocking(true),
      _local_ep(),
      _remote_ep(),
      _nm(nm),
      _channel(caps),
      _recv_queue() {
}

Socket::~Socket() {
    _nm.remove_socket(this);
}

void Socket::tear_down() {
    try {
        // we have no connection to tear down here, but only want to make sure that all packets we sent
        // are seen and handled by the server. thus, wait until we have got all replies to our
        // potentially in-flight packets, in which case we also have received our credits back.
        while(!_channel.has_all_credits())
            wait_for_credits();
    }
    catch(...) {
        // ignore errors
    }
}

void Socket::disconnect() {
    _state = Closed;
    _local_ep = Endpoint();
    _remote_ep = Endpoint();
}

void Socket::process_message(const NetEventChannel::ControlMessage &message,
                             NetEventChannel::Event &event) {
    switch(message.type) {
        case NetEventChannel::Data:
            return handle_data(static_cast<NetEventChannel::DataMessage const &>(message), event);
        case NetEventChannel::Connected:
            return handle_connected(static_cast<NetEventChannel::ConnectedMessage const &>(message));
        case NetEventChannel::Closed:
            return handle_closed(static_cast<NetEventChannel::ClosedMessage const &>(message));
        case NetEventChannel::CloseReq:
            return handle_close_req(static_cast<NetEventChannel::CloseReqMessage const &>(message));
        default:
            throw Exception(Errors::NOT_SUP);
    }
}

void Socket::handle_data(NetEventChannel::DataMessage const &msg, NetEventChannel::Event &event) {
    LLOG(NET, "socket " << _sd << ": received data with " << msg.size << "b"
                              << " from " << IpAddr(msg.addr) << ":" << msg.port);
    _recv_queue.append(new DataQueue::Item(&msg, std::move(event)));
}

void Socket::handle_connected(NetEventChannel::ConnectedMessage const &msg) {
    LLOG(NET, "socket " << _sd << ": connected to " << IpAddr(msg.addr) << ":" << msg.port);
    _state = Connected;
    _remote_ep.addr = IpAddr(msg.addr);
    _remote_ep.port = msg.port;
}

void Socket::handle_close_req(NetEventChannel::CloseReqMessage const &) {
    LLOG(NET, "socket " << _sd << ": remote side was closed");
    _state = RemoteClosed;
}

void Socket::handle_closed(NetEventChannel::ClosedMessage const &) {
    LLOG(NET, "socket " << _sd << ": closed");
    disconnect();
}

bool Socket::get_next_data(const uchar **data, size_t *size, Endpoint *ep) {
    while(true) {
        if(_recv_queue.get_next_data(data, size, ep))
            return true;

        if(_state == Closed)
            throw Exception(Errors::INV_STATE);
        if(!_blocking) {
            process_events();
            return false;
        }

        wait_for_events();
    }
}

ssize_t Socket::do_recv(void *dst, size_t amount, Endpoint *ep) {
    const uchar *pkt_data = nullptr;
    size_t pkt_size = 0;
    if(!get_next_data(&pkt_data, &pkt_size, ep))
        return -1;

    size_t msg_size = Math::min(pkt_size, amount);
    memcpy(dst, pkt_data, msg_size);

    // ack read data and discard excess bytes that do not fit into the supplied buffer
    ack_data(msg_size);

    return static_cast<ssize_t>(msg_size);
}

ssize_t Socket::do_send(const void *src, size_t amount, const Endpoint &ep) {
    while(true) {
        bool succeeded = _channel.send_data(ep, amount, [src, amount](void *buf) {
            memcpy(buf, src, amount);
        });
        if(succeeded)
            return static_cast<ssize_t>(amount);

        if(!blocking()) {
            fetch_replies();
            return -1;
        }

        wait_for_credits();

        if(_state == Closed)
            throw Exception(Errors::SOCKET_CLOSED);
    }
}

void Socket::ack_data(size_t size) {
    _recv_queue.ack_data(size);
}

void Socket::wait_for_events() {
    while(!process_events())
        _channel.wait_for_events();
}

void Socket::wait_for_credits() {
    while(true) {
        fetch_replies();
        if(can_send())
            break;
        _channel.wait_for_credits();
    }
}

bool Socket::process_events() {
    bool seen_event = false;
    for(int i = 0; i < EVENT_FETCH_BATCH_SIZE; i++) {
        auto event = _channel.recv_message();
        if(!event.is_present())
            break;

        auto message = static_cast<NetEventChannel::ControlMessage const *>(event.get_message());
        process_message(*message, event);
        seen_event = true;
    }
    return seen_event;
}

void Socket::fetch_replies() {
    _channel.fetch_replies();
}

bool Socket::can_send() {
    return _channel.can_send();
}

}
