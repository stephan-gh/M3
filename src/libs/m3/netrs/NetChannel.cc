/*
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
#include <m3/netrs/NetChannel.h>

namespace m3 {

NetChannel::NetChannel(capsel_t caps)
    : _sg(SendGate::bind(caps + 1, nullptr)),
      _rg(RecvGate::bind(caps + 0, nextlog2<MSG_BUF_SIZE>::val, nextlog2<MSG_SIZE>::val)),
      _mem(MemGate::bind(caps + 2)) {
    // Activate the rgate manually
    _rg.activate();
}

void NetChannel::send(m3::net::NetData data) {
    LLOG(NET, "NetLogSend:");
    data.log();
    _sg.send_aligned(&data, data.send_size());
}

m3::net::NetData *NetChannel::receive() {
    const TCU::Message *msg = _rg.fetch();
    if(msg != nullptr) {
        LLOG(NET, "msglength=" << msg->length << " sizeof=" << sizeof(m3::net::NetData));
        // this is an actual package, therefore copy the data into a buffer thats cast
        // into the NetData struct
        m3::net::NetData *package = new m3::net::NetData();
        // TODO Somehow prevent copy?
        memcpy(static_cast<void *>(package), msg->data, msg->length);
        // package->log();
        // Ack message to free channel
        _rg.ack_msg(msg);
        return package;
    }
    else {
        return nullptr;
    }
}

}
