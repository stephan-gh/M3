/*
 * Copyright (C) 2021-2022 Nils Asmussen, Barkhausen Institut
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

#include <base/log/Lib.h>
#include <m3/net/NetEventChannel.h>
#include <m3/tiles/Activity.h>

namespace m3 {

NetEventChannel::NetEventChannel(capsel_t caps)
    : _rgate(RecvGate::bind(caps + 0, nextlog2<MSG_BUF_SIZE>::val, nextlog2<MSG_SIZE>::val)),
      _rplgate(RecvGate::create(nextlog2<REPLY_BUF_SIZE>::val, nextlog2<REPLY_SIZE>::val)),
      _sgate(SendGate::bind(caps + 1, &_rplgate)) {
    _rgate.activate();
    _rplgate.activate();
}

Errors::Code NetEventChannel::send_data(const Endpoint &ep, size_t size,
                                        std::function<void(uchar *)> cb_data) {
    // make sure that the message does not contain a page boundary
    ALIGNED(2048) char msg_buf[2048];
    if(size > sizeof(msg_buf) - sizeof(DataMessage))
        return Errors::OUT_OF_BOUNDS;

    auto msg = reinterpret_cast<DataMessage*>(msg_buf);
    msg->type = Data;
    msg->addr = static_cast<uint64_t>(ep.addr.addr());
    msg->port = static_cast<uint64_t>(ep.port);
    msg->size = static_cast<uint64_t>(size);
    cb_data(msg->data);

    fetch_replies();

    return _sgate.try_send_aligned(msg_buf, size + sizeof(DataMessage));
}

bool NetEventChannel::send_close_req() {
    MsgBuf msg_buf;
    auto &msg = msg_buf.cast<CloseReqMessage>();
    msg.type = CloseReq;
    return _sgate.try_send(msg_buf) == Errors::NONE;
}

bool NetEventChannel::can_send() const noexcept {
    return _sgate.can_send();
}

bool NetEventChannel::has_events() const noexcept {
    return _rgate.has_msgs();
}

bool NetEventChannel::has_all_credits() {
    return _sgate.credits() == MSG_CREDITS;
}

NetEventChannel::Event NetEventChannel::recv_message() {
    return Event(_rgate.fetch(), this);
}

void NetEventChannel::wait_for_events() {
    _rgate.wait_for_msg();
}

void NetEventChannel::wait_for_credits() {
    _rplgate.wait_for_msg();
}

void NetEventChannel::fetch_replies() {
    auto reply = _rplgate.fetch();
    while(reply != nullptr) {
        _rplgate.ack_msg(reply);
        reply = _rplgate.fetch();
    }
}

NetEventChannel::Event::Event() noexcept
    : _msg(nullptr),
       _channel(nullptr),
       _ack(false) {
}

NetEventChannel::Event::~Event() {
    try {
        finish();
    }
    catch(...) {
        // ignore
    }
}

NetEventChannel::Event::Event(NetEventChannel::Event&& e) noexcept
    : _msg(e._msg),
      _channel(e._channel),
      _ack(e._ack) {
    e._ack = false;
}

NetEventChannel::Event& NetEventChannel::Event::operator =(NetEventChannel::Event&& e) noexcept {
    _msg = e._msg;
    _channel = e._channel;
    _ack = e._ack;
    e._ack = false;
    return *this;
}

bool NetEventChannel::Event::is_present() noexcept {
    return _msg;
}

void NetEventChannel::Event::finish() {
    if(is_present() && _ack) {
        // give credits back with empty message
        MsgBuf msg_buf;
        _channel->_rgate.reply(msg_buf, _msg);
        _ack = false;
    }
}

const NetEventChannel::ControlMessage* NetEventChannel::Event::get_message() noexcept {
    return reinterpret_cast<const NetEventChannel::ControlMessage *>(_msg->data);
}

NetEventChannel::Event::Event(const TCU::Message *msg, NetEventChannel *channel) noexcept
    : _msg(msg),
      _channel(channel),
      _ack(true) {
}

}
