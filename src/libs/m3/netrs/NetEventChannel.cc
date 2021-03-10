/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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
#include <m3/netrs/NetEventChannel.h>
#include <m3/pes/VPE.h>

namespace m3 {

NetEventChannelRs::NetEventChannelRs(capsel_t caps)
    : _rgate(RecvGate::bind(caps + 0, nextlog2<MSG_BUF_SIZE>::val, nextlog2<MSG_SIZE>::val)),
      _rplgate(RecvGate::create(nextlog2<REPLY_BUF_SIZE>::val, nextlog2<REPLY_SIZE>::val)),
      _sgate(SendGate::bind(caps + 1, &_rplgate)) {
    _rgate.activate();
    _rplgate.activate();
}

bool NetEventChannelRs::send_data(int sd, IpAddr addr, uint16_t port, size_t size, std::function<void(uchar *)> cb_data) {
    LLOG(NET, "NetEventChannel::data(sd=" << sd << ", size=" << size << ")");

    // make sure that the message does not contain a page boundary
    ALIGNED(2048) char msg_buf[2048];
    auto msg = reinterpret_cast<DataMessage*>(msg_buf);
    msg->type = Data;
    msg->sd = static_cast<uint64_t>(sd);
    msg->addr = static_cast<uint64_t>(addr.addr());
    msg->port = static_cast<uint64_t>(port);
    msg->size = static_cast<uint64_t>(size);
    cb_data(msg->data);

    fetch_replies();

    return _sgate.try_send_aligned(msg_buf, size + sizeof(DataMessage)) == Errors::NONE;
}

bool NetEventChannelRs::send_close_req(int sd) {
    MsgBuf msg_buf;
    auto &msg = msg_buf.cast<CloseReqMessage>();
    msg.type = CloseReq;
    msg.sd = static_cast<uint64_t>(sd);
    return _sgate.try_send(msg_buf) == Errors::NONE;
}

bool NetEventChannelRs::has_events() const {
    return _rgate.has_msgs();
}

NetEventChannelRs::Event NetEventChannelRs::recv_message() {
    return Event(_rgate.fetch(), this);
}

void NetEventChannelRs::fetch_replies() {
    auto reply = _rplgate.fetch();
    while(reply != nullptr) {
        _rplgate.ack_msg(reply);
        reply = _rplgate.fetch();
    }
}

NetEventChannelRs::Event::Event() noexcept
    : _msg(nullptr),
       _channel(nullptr),
       _ack(false) {
}

NetEventChannelRs::Event::~Event() {
    try {
        finish();
    }
    catch(...) {
        // ignore
    }
}

NetEventChannelRs::Event::Event(NetEventChannelRs::Event&& e) noexcept
    : _msg(e._msg),
      _channel(e._channel),
      _ack(e._ack) {
    e._ack = false;
}

NetEventChannelRs::Event& NetEventChannelRs::Event::operator =(NetEventChannelRs::Event&& e) noexcept {
    _msg = e._msg;
    _channel = e._channel;
    _ack = e._ack;
    e._ack = false;
    return *this;
}

bool NetEventChannelRs::Event::is_present() noexcept {
    return _msg;
}

void NetEventChannelRs::Event::finish() {
    if(is_present() && _ack) {
        // Only acknowledge message
        _channel->_rgate.ack_msg(_msg);
        _ack = false;
    }
}

const NetEventChannelRs::ControlMessage* NetEventChannelRs::Event::get_message() noexcept {
    return reinterpret_cast<const NetEventChannelRs::ControlMessage *>(_msg->data);
}

NetEventChannelRs::Event::Event(const TCU::Message *msg, NetEventChannelRs *channel) noexcept
    : _msg(msg),
      _channel(channel),
      _ack(true) {
}

}
