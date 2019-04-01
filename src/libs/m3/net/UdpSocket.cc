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

#include <base/Heap.h>

#include <m3/net/UdpSocket.h>
#include <m3/session/NetworkManager.h>

namespace m3 {

UdpSocket::UdpSocket(int sd, NetworkManager& nm)
    : Socket(sd, nm)
{
}

UdpSocket::~UdpSocket() {
}

Socket::SocketType UdpSocket::type() {
    return SOCK_DGRAM;
}

Errors::Code UdpSocket::connect(IpAddr addr, uint16_t port) {
    // TODO: Allow UdpSocket to be reconnected to a different remote socket?
    if(_state != None && _state != Connected)
        return inv_state();

    auto result = _nm.connect(sd(), addr, port);
    if(result == Errors::NONE) {
        _remote_addr = addr;
        _remote_port = port;
    }
    return update_status(result, Connected);
}

ssize_t UdpSocket::sendto(const void *src, size_t amount, IpAddr dst_addr, uint16_t dst_port) {
    // The write of header and data needs to be an "atomic" action
    size_t size = MessageHeader::serialize_length() + amount;

    while(_state != Closed) {
        auto err = _channel->inband_data_transfer(_sd, size, [&](uchar * buf) {
            Marshaller m(buf, MessageHeader::serialize_length());
            MessageHeader hdr(dst_addr, dst_port, amount);
            hdr.serialize(m);
            memcpy(buf + MessageHeader::serialize_length(), src, amount);
        });

        if(err == Errors::NONE)
            return static_cast<ssize_t>(amount);

        if(err != Errors::MISS_CREDITS || !_blocking) {
            Errors::last = err;
            return -1;
        }

        // Block until channel regains credits.
        wait_for_credit();
    };

    Errors::last = inv_state();
    return -1;
}

ssize_t UdpSocket::recvmsg(void *dst, size_t amount, IpAddr *src_addr, uint16_t *src_port) {
    const uchar * pkt_data = nullptr;
    size_t pkt_size = 0;
    Errors::last = get_next_data(pkt_data, pkt_size);
    if(Errors::last != Errors::NONE)
        return -1;

    MessageHeader hdr;
    Unmarshaller um(pkt_data, pkt_size);
    assert(hdr.serialize_length() <= um.length());
    hdr.unserialize(um);

    if(src_addr)
        *src_addr = hdr.addr;
    if(src_port)
        *src_port = hdr.port;

    size_t msg_size = Math::min(hdr.size, amount);
    assert(msg_size <= um.remaining());
    // cout << "recvmsg: hdr.size=" << hdr.size << ", msg_size=" << msg_size << "\n";
    memcpy(dst, um.buffer() + um.pos(), msg_size);
    // cout << "recvmsg: read_size=" << read_size << "\n";

    // ack read data and discard excess bytes that do not fit into the supplied buffer
    ack_data(um.pos() + hdr.size);

    return static_cast<ssize_t>(msg_size);
}

}
