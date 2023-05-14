/*
 * Copyright (C) 2021-2022 Nils Asmussen, Barkhausen Institut
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

#include <base/Log.h>
#include <base/time/Instant.h>

#include <m3/Exception.h>
#include <m3/com/GateStream.h>
#include <m3/com/OpCodes.h>
#include <m3/net/Socket.h>
#include <m3/session/Network.h>
#include <m3/stream/Standard.h>

#include <thread/ThreadManager.h>

namespace m3 {

Network::Network(const std::string_view &service) : ClientSession(service), _sgate(connect()) {
}

int32_t Network::create(SocketType type, uint8_t protocol, const SocketArgs &args, capsel_t *caps) {
    KIF::ExchangeArgs eargs;
    ExchangeOStream os(eargs);
    os << opcodes::Net::CREATE << static_cast<uint64_t>(type) << protocol << args.rbuf_size
       << args.rbuf_slots << args.sbuf_size << args.sbuf_slots;
    eargs.bytes = os.total();
    KIF::CapRngDesc crd = obtain(2, &eargs);
    *caps = crd.start();

    int32_t sd;
    ExchangeIStream is(eargs);
    is >> sd;
    return sd;
}

IpAddr Network::ip_addr() {
    GateIStream reply = send_receive_vmsg(_sgate, opcodes::Net::GET_IP);
    reply.pull_result();
    uint32_t addr;
    reply >> addr;
    return IpAddr(addr);
}

IpAddr Network::get_nameserver() {
    GateIStream reply = send_receive_vmsg(_sgate, opcodes::Net::GET_NAMESRV);
    reply.pull_result();
    uint32_t addr;
    reply >> addr;
    return IpAddr(addr);
}

std::pair<IpAddr, port_t> Network::bind(int32_t sd, port_t port) {
    GateIStream reply = send_receive_vmsg(_sgate, opcodes::Net::BIND, sd, port);
    reply.pull_result();
    uint32_t addr;
    reply >> addr >> port;
    return std::make_pair(IpAddr(addr), port);
}

IpAddr Network::listen(int32_t sd, port_t port) {
    GateIStream reply = send_receive_vmsg(_sgate, opcodes::Net::LISTEN, sd, port);
    reply.pull_result();
    uint32_t addr;
    reply >> addr;
    return IpAddr(addr);
}

Endpoint Network::connect_socket(int32_t sd, Endpoint remote_ep) {
    GateIStream reply =
        send_receive_vmsg(_sgate, opcodes::Net::CONNECT, sd, remote_ep.addr.addr(), remote_ep.port);
    reply.pull_result();
    uint32_t addr;
    port_t port;
    reply >> addr >> port;
    return Endpoint(IpAddr(addr), port);
}

void Network::abort(int32_t sd, bool remove) {
    GateIStream reply = send_receive_vmsg(_sgate, opcodes::Net::ABORT, sd, remove);
    reply.pull_result();
}

} // namespace m3
