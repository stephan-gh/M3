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

SocketRs::SocketRs(int sd, NetworkManagerRs &nm)
    : TreapNode(sd),
      RefCounted(),
      _sd(sd),
      _state(Closed),
      _close_cause(Errors::NONE),
      _blocking(true),
      _local_addr(IpAddr(0, 0, 0, 0)),
      _local_port(0),
      _remote_addr(IpAddr(0, 0, 0, 0)),
      _remote_port(0),
      _nm(nm),
      _recv_queue() {
}

void SocketRs::set_local(IpAddr addr, uint16_t port, State state) {
    _local_addr = addr;
    _local_port = port;
    _state = state;
}

void SocketRs::process_message(const NetEventChannelRs::SocketControlMessage &message,
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

void SocketRs::handle_data(NetEventChannelRs::DataMessage const & msg, NetEventChannelRs::Event &event) {
    _recv_queue.append(new DataQueueRs::Item(&msg, std::move(event)));
}

void SocketRs::handle_connected(NetEventChannelRs::ConnectedMessage const &msg) {
    _state = Connected;
    _remote_addr = IpAddr(msg.addr);
    _remote_port = msg.port;
}

void SocketRs::handle_close_req(NetEventChannelRs::CloseReqMessage const &) {
    _state = Closing;
}

void SocketRs::handle_closed(NetEventChannelRs::ClosedMessage const &) {
    _state = Closed;
}

bool SocketRs::get_next_data(const uchar **data, size_t *size, IpAddr *src_addr, uint16_t *src_port) {
    while(true) {
        process_events();

        if(_recv_queue.get_next_data(data, size, src_addr, src_port))
            return true;

        if(_state == Closed)
            inv_state();
        if(!_blocking)
            return false;

        wait_for_event();
    }
}

ssize_t SocketRs::recvfrom(void *dst, size_t amount, IpAddr *src_addr, uint16_t *src_port) {
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

ssize_t SocketRs::sendto(const void *src, size_t amount, IpAddr dst_addr, uint16_t dst_port) {
    while(true) {
        ssize_t res = _nm.send(_sd, dst_addr, dst_port, src, amount);
        if(res != -1)
            return res;

        if(!blocking())
            return -1;

        _nm.wait_sync();

        process_events();

        if(_state == Closed)
          inv_state();
    }
}

void SocketRs::ack_data(size_t size) {
    _recv_queue.ack_data(size);
}

void SocketRs::process_events() {
    for(int i = 0; i < EVENT_FETCH_BATCH_SIZE; i++) {
        auto event = _nm.recv_event();
        if(!event.is_present())
            break;
        // Stop once we received a message for this socket.
        if(_nm.process_event(event) == this)
            break;
    }
}

void SocketRs::wait_for_event() {
    _nm.wait_sync();
}

void SocketRs::inv_state() {
    or_closed(Errors::INV_STATE);
}

void SocketRs::or_closed(Errors::Code err) {
    if(_state == Closed)
        err = _close_cause != Errors::NONE ? _close_cause : Errors::SOCKET_CLOSED;
    throw Exception(err);
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
