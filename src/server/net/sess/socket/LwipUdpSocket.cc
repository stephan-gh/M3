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

#include "lwip/ip_addr.h"
#include "lwip/pbuf.h"
#include "lwip/udp.h"

#include "LwipUdpSocket.h"

using namespace m3;

m3::Errors::Code LwipUdpSocket::create(uint8_t protocol) {
    if(protocol != 0 && protocol != IP_PROTO_UDP) {
        LOG_SOCKET(this, "create failed: invalid protocol");
        return Errors::INV_ARGS;
    }
    _pcb = udp_new();
    if(!_pcb) {
        LOG_SOCKET(this, "create failed: allocation of pcb failed");
        return Errors::NO_SPACE;
    }

    // set argument for callback functions
    udp_recv(_pcb, udp_recv_cb, this);

    return Errors::NONE;
}

ssize_t LwipUdpSocket::send_data(const void * data, size_t size) {
    assert(MessageHeader::serialize_length() <= size);

    MessageHeader hdr;
    Unmarshaller um(static_cast<const unsigned char *>(data), size);
    hdr.unserialize(um);
    ip_addr_t addr =  IPADDR4_INIT(lwip_htonl(hdr.addr.addr()));

    LOG_SOCKET(this, "UdpSocket::send_data(): port=" << hdr.port << ", size=" << hdr.size);

    if(hdr.size != um.remaining()) {
        LOG_SOCKET(this, "UdpSocket::send_data(): hdr.size != remaining size");
        return -1;
    }

    struct pbuf *p = pbuf_alloc(PBUF_TRANSPORT, hdr.size, PBUF_RAM);
    if(p) {
       err_t err = pbuf_take(p, um.buffer() + um.pos(), hdr.size);
       if(err == ERR_OK) {
           if(ip_addr_cmp(&addr, IP_ADDR_ANY))
               err = udp_send(_pcb, p);
           else
               err = udp_sendto(_pcb, p, &addr, hdr.port);
           if(err != ERR_OK)
               LOG_SOCKET(this, "UdpSocket::send_data(): udp_send failed: " << errToStr(err));
       }
       else
           LOG_SOCKET(this, "UdpSocket::send_data(): failed to read message data: " << errToStr(err));
       pbuf_free(p);
   }
   else {
       LOG_SOCKET(this, "UdpSocket::send_data(): failed to allocate pbuf, dropping udp packet");
   }
    return static_cast<ssize_t>(size);
}

void LwipUdpSocket::udp_recv_cb(void *arg, struct udp_pcb*, struct pbuf *p, const ip_addr_t *addr, u16_t port) {
    // TODO: avoid unnecessary copy operation
    auto socket = static_cast<LwipUdpSocket *>(arg);

    size_t size = p->tot_len + MessageHeader::serialize_length();
    LOG_SOCKET(socket, "udp_recv_cb: size " << size);
    LOG_SOCKET(socket, "udp_recv_cb: offset " << MessageHeader::serialize_length());
    Errors::Code err = socket->_channel->inband_data_transfer(socket->_sd, size, [&](uchar * buf) {
        Marshaller m(buf, MessageHeader::serialize_length());
        MessageHeader hdr(IpAddr(lwip_ntohl(addr->addr)), port, p->tot_len);
        hdr.serialize(m);
        pbuf_copy_partial(p, buf + MessageHeader::serialize_length(), p->tot_len, 0);
        LOG_SOCKET(socket, "udp_recv_cb: forwarding data to user (" << p->tot_len << ")");
    });

    if(err != Errors::NONE)
        LOG_SOCKET(socket, "udp_recv_cb: recv_pipe is full, dropping datagram: " << err);

    pbuf_free(p);
}


m3::Errors::Code LwipUdpSocket::bind(ip4_addr addr, uint16_t port) {
    err_t err = udp_bind(_pcb, &addr, port);
    if(err != ERR_OK)
        LOG_SOCKET(this, "bind failed: " << errToStr(err));

    return(mapError(err));
}

m3::Errors::Code LwipUdpSocket::listen() {
    LOG_SOCKET(this, "listen failed: not a stream socket");
    return Errors::INV_ARGS;
}

m3::Errors::Code LwipUdpSocket::connect(ip4_addr addr, uint16_t port) {
    err_t err = udp_connect(_pcb, &addr, port);
    if(err != ERR_OK)
        LOG_SOCKET(this, "connect failed: " << errToStr(err));
    return mapError(err);
}

m3::Errors::Code LwipUdpSocket::close() {
    udp_remove(_pcb);
    _pcb = nullptr;
    return Errors::NONE;
}
