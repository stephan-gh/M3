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

#include <base/log/Lib.h>

#include <m3/netrs/Net.h>
#include <m3/netrs/Socket.h>
#include <m3/netrs/TcpSocket.h>
#include <m3/netrs/UdpSocket.h>
#include <m3/session/NetworkManagerRs.h>

namespace m3 {

SocketRs::SocketRs(int sd, capsel_t caps, NetworkManagerRs &nm)
    : SListItem(),
      RefCounted(),
      _sd(sd),
      _state(Closed),
      _blocking(true),
      _local_addr(IpAddr(0, 0, 0, 0)),
      _local_port(0),
      _remote_addr(IpAddr(0, 0, 0, 0)),
      _remote_port(0),
      _nm(nm),
      _channel(caps),
      _recv_queue() {
}

void SocketRs::set_local(IpAddr addr, port_t port, State state) {
    _local_addr = addr;
    _local_port = port;
    _state = state;
}

void SocketRs::process_message(const NetEventChannelRs::ControlMessage &message,
                               NetEventChannelRs::Event &event) {
    switch(message.type) {
        case NetEventChannelRs::Data:
            return handle_data(static_cast<NetEventChannelRs::DataMessage const &>(message), event);
        case NetEventChannelRs::Connected:
            return handle_connected(static_cast<NetEventChannelRs::ConnectedMessage const &>(message));
        case NetEventChannelRs::Closed:
            return handle_closed(static_cast<NetEventChannelRs::ClosedMessage const &>(message));
        case NetEventChannelRs::CloseReq:
            return handle_close_req(static_cast<NetEventChannelRs::CloseReqMessage const &>(message));
        default:
            throw Exception(Errors::NOT_SUP);
    }
}

void SocketRs::handle_data(NetEventChannelRs::DataMessage const &msg, NetEventChannelRs::Event &event) {
    LLOG(NET, "socket " << _sd << ": received data with " << msg.size << "b"
                              << " from " << IpAddr(msg.addr) << ":" << msg.port);
    _recv_queue.append(new DataQueueRs::Item(&msg, std::move(event)));
}

void SocketRs::handle_connected(NetEventChannelRs::ConnectedMessage const &msg) {
    LLOG(NET, "socket " << _sd << ": connected to " << IpAddr(msg.addr) << ":" << msg.port);
    _state = Connected;
    _remote_addr = IpAddr(msg.addr);
    _remote_port = msg.port;
}

void SocketRs::handle_close_req(NetEventChannelRs::CloseReqMessage const &) {
    LLOG(NET, "socket " << _sd << ": remote side was closed");
    _state = RemoteClosed;
}

void SocketRs::handle_closed(NetEventChannelRs::ClosedMessage const &) {
    LLOG(NET, "socket " << _sd << ": closed");
    _state = Closed;
}

bool SocketRs::get_next_data(const uchar **data, size_t *size, IpAddr *src_addr, port_t *src_port) {
    while(true) {
        if(_recv_queue.get_next_data(data, size, src_addr, src_port))
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

ssize_t SocketRs::do_recv(void *dst, size_t amount, IpAddr *src_addr, port_t *src_port) {
    const uchar *pkt_data = nullptr;
    size_t pkt_size = 0;
    if(!get_next_data(&pkt_data, &pkt_size, src_addr, src_port))
        return -1;

    size_t msg_size = Math::min(pkt_size, amount);
    memcpy(dst, pkt_data, msg_size);

    // ack read data and discard excess bytes that do not fit into the supplied buffer
    ack_data(msg_size);

    return static_cast<ssize_t>(msg_size);
}

ssize_t SocketRs::do_send(const void *src, size_t amount, IpAddr dst_addr, port_t dst_port) {
    while(true) {
        bool succeeded = _channel.send_data(dst_addr, dst_port, amount, [src, amount](void *buf) {
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

void SocketRs::ack_data(size_t size) {
    _recv_queue.ack_data(size);
}

void SocketRs::wait_for_events() {
    while(!process_events())
        _channel.wait_for_events();
}

void SocketRs::wait_for_credits() {
    while(true) {
        fetch_replies();
        if(can_send())
            break;
        _channel.wait_for_credits();
    }
}

bool SocketRs::process_events() {
    bool seen_event = false;
    for(int i = 0; i < EVENT_FETCH_BATCH_SIZE; i++) {
        auto event = _channel.recv_message();
        if(!event.is_present())
            break;

        auto message = static_cast<NetEventChannelRs::ControlMessage const *>(event.get_message());
        process_message(*message, event);
        seen_event = true;
    }
    return seen_event;
}

void SocketRs::fetch_replies() {
    _channel.fetch_replies();
}

bool SocketRs::can_send() {
    return _channel.can_send();
}

void SocketRs::abort() {
    do_abort(false);
}

void SocketRs::do_abort(bool remove) {
    _nm.abort(sd(), remove);
    // Clear receive queue before potentially destroying the channel,
    // because the queue contains events that point to the channel.
    _recv_queue.clear();
    _state = State::Closed;
}

}
