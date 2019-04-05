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
#include <m3/net/NetEventChannel.h>
#include <m3/VPE.h>

namespace m3 {

void NetEventChannel::prepare_caps(capsel_t caps, size_t size) {
    RecvGate rgate_srv(RecvGate::create_for(VPE::self(), caps + 0, nextlog2<MSG_BUF_SIZE>::val,
            nextlog2<MSG_SIZE>::val, RecvGate::KEEP_CAP));
    RecvGate rgate_cli(RecvGate::create_for(VPE::self(), caps + 3, nextlog2<MSG_BUF_SIZE>::val,
                nextlog2<MSG_SIZE>::val, RecvGate::KEEP_CAP));
    SendGate sgate_srv(SendGate::create(&rgate_cli, 0, SendGate::UNLIMITED, &rgate_srv, caps + 1, MemGate::KEEP_CAP));
    SendGate sgate_cli(SendGate::create(&rgate_srv, 0, MSG_CREDITS, &rgate_cli, caps + 4, MemGate::KEEP_CAP));
    MemGate mem_srv(MemGate::create_global(2 * size, MemGate::RW, caps + 2, MemGate::KEEP_CAP));
    MemGate mem_cli(mem_srv.derive_for(VPE::self().sel(), caps + 5, 0, 2 * size, mem_srv.RW, MemGate::KEEP_CAP));
}

NetEventChannel::NetEventChannel(capsel_t caps, bool ret_credits)
    : _ret_credits(ret_credits),
      _rgate(RecvGate::bind(caps + 0, nextlog2<MSG_BUF_SIZE>::val)),
      _sgate(SendGate::bind(caps + 1, &RecvGate::invalid())),
      _workitem(nullptr),_credit_event(0), _waiting_credit(0) {
}

NetEventChannel::~NetEventChannel() {
    stop();
}

Errors::Code NetEventChannel::data_transfer(int sd, size_t pos, size_t size) {
    LLOG(NET, "NetEventChannel::data_transfer(sd=" << sd << ", pos=" << pos << ", size=" << size << ")");
    NetEventChannel::DataTransferMessage msg;
    msg.type = DataTransfer;
    msg.sd = sd;
    msg.pos = pos;
    msg.size = size;
    return send_message(&msg, sizeof(msg));
}

Errors::Code NetEventChannel::ack_data_transfer(int sd, size_t pos, size_t size) {
    LLOG(NET, "NetEventChannel::ack_data_transfer(sd=" << sd << ", pos=" << pos << ", size=" << size << ")");
    NetEventChannel::AckDataTransferMessage msg;
    msg.type = AckDataTransfer;
    msg.sd = sd;
    msg.pos = pos;
    msg.size = size;
    return send_message(&msg, sizeof(msg));
}

Errors::Code NetEventChannel::inband_data_transfer(int sd, size_t size, std::function<void(uchar *)> cb_data) {
    LLOG(NET, "NetEventChannel::inband_data_transfer(sd=" << sd << ", size=" << size << ")");
    // TODO: Avoid allocation and copy
    void * msg_data = malloc(size + sizeof(InbandDataTransferMessage));
    auto msg = static_cast<InbandDataTransferMessage *>(msg_data);
    msg->type = InbandDataTransfer;
    msg->sd = sd;
    msg->size = size;
    cb_data(msg->data);

    // TODO: Send via seperate send/receive gate?
    Errors::Code result = send_message(msg_data, size + sizeof(InbandDataTransferMessage));
    if(result != Errors::NONE)
        LLOG(NET, "NetEventChannel::inband_data_transfer() failed: " << Errors::to_string(result));

    free(msg_data);
    return result;
}

Errors::Code NetEventChannel::socket_accept(int sd, int new_sd, IpAddr remote_addr, uint16_t remote_port) {
    LLOG(NET, "NetEventChannel::socket_accept(sd=" << sd << ", new_sd=" << new_sd << ")");
    NetEventChannel::SocketAcceptMessage msg;
    msg.type = SocketAccept;
    msg.sd = sd;
    msg.new_sd = new_sd;
    msg.remote_addr = remote_addr;
    msg.remote_port = remote_port;
    return send_message(&msg, sizeof(msg));
}


Errors::Code NetEventChannel::socket_connected(int sd) {
    LLOG(NET, "NetEventChannel::socket_connected(sd=" << sd << ")");
    NetEventChannel::SocketConnectedMessage msg;
    msg.type = SocketConnected;
    msg.sd = sd;
    return send_message(&msg, sizeof(msg));
}

Errors::Code NetEventChannel::socket_closed(int sd, Errors::Code cause) {
    LLOG(NET, "NetEventChannel::socket_closed(sd=" << sd << ")");
    NetEventChannel::SocketClosedMessage msg;
    msg.type = SocketClosed;
    msg.sd = sd;
    msg.cause = cause;
    return send_message(&msg, sizeof(msg));
}

Errors::Code NetEventChannel::send_message(const void* msg, size_t size) {
    return _sgate.send(msg, size);
}

void NetEventChannel::start(evhandler_t evhandler, crdhandler_t crdhandler) {
    if(!_workitem) {
        _evhandler = evhandler;
        _crdhandler = crdhandler;
        _workitem = new EventWorkItem(this);
        env()->workloop()->add(_workitem, false);
    }
}

void NetEventChannel::stop() {
    if(_workitem) {
        env()->workloop()->remove(_workitem);
        delete _workitem;
        _workitem = nullptr;
    }
}

NetEventChannel::Event NetEventChannel::recv_message() {
    return Event(_rgate.fetch(), this);
}

bool NetEventChannel::has_credits() {
    return _sgate.ep() == SendGate::UNBOUND || DTU::get().has_credits(_sgate.ep());
}

void NetEventChannel::set_credit_event(event_t event) {
    _credit_event = event;
}

event_t NetEventChannel::get_credit_event() {
    return _credit_event;
}

void NetEventChannel::wait_for_credit() {
    _waiting_credit++;
}

NetEventChannel::Event::Event()
    : _msg(nullptr),
       _channel(nullptr),
       _ack(false) {
}

NetEventChannel::Event::~Event() {
    finish();
}

NetEventChannel::Event::Event(NetEventChannel::Event&& e)
    : _msg(e._msg),
      _channel(e._channel),
      _ack(e._ack) {
    e._ack = false;
}

NetEventChannel::Event& NetEventChannel::Event::operator =(NetEventChannel::Event&& e) {
    _msg = e._msg;
    _channel = e._channel;
    _ack = e._ack;
    e._ack = false;
    return *this;
}

bool NetEventChannel::Event::is_present() {
    return _msg;
}

void NetEventChannel::Event::finish() {
    if(is_present() && _ack) {
        auto msgoff = DTU::get().get_msgoff(_channel->_rgate.ep(), _msg);
        if(_channel->_ret_credits) {
            auto data = 0;
            if(_channel->_rgate.reply(&data, sizeof(data), msgoff) != Errors::NONE)
                LLOG(NET, "Unable to give credits back: " << Errors::last);
        } else {
            // Only acknowledge message
            DTU::get().mark_read(_channel->_rgate.ep(), msgoff);
        }
        _ack = false;
    }
}

GateIStream NetEventChannel::Event::to_stream() {
    GateIStream stream(_channel->_rgate, _msg);
    stream.claim();
    return stream;
}


const NetEventChannel::ControlMessage* NetEventChannel::Event::get_message() {
    return reinterpret_cast<const NetEventChannel::ControlMessage *>(_msg->data);
}

NetEventChannel::Event::Event(const DTU::Message *msg, NetEventChannel *channel)
    : _msg(msg),
      _channel(channel),
      _ack(true) {
}

void NetEventChannel::EventWorkItem::work() {
    Event event = _channel->recv_message();
    if(event.is_present())
        _channel->_evhandler(event);

    if(_channel->_waiting_credit && _channel->has_credits())
    {
        auto waiting_credit = _channel->_waiting_credit;
        _channel->_waiting_credit = 0;
        _channel->_crdhandler(_channel->_credit_event, waiting_credit);
    }
}

}
