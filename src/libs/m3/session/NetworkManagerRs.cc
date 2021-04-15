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

#include <m3/Exception.h>
#include <m3/com/GateStream.h>
#include <m3/netrs/Socket.h>
#include <m3/session/NetworkManagerRs.h>
#include <m3/stream/Standard.h>

#include <thread/ThreadManager.h>

namespace m3 {

KIF::CapRngDesc NetworkManagerRs::get_sgate(ClientSession &sess) {
    KIF::ExchangeArgs eargs;
    ExchangeOStream os(eargs);
    os << Operation::GET_SGATE;
    eargs.bytes = os.total();
    return sess.obtain(1, &eargs);
}

NetworkManagerRs::NetworkManagerRs(const String &service)
    : ClientSession(service),
      _metagate(SendGate::bind(get_sgate(*this).start())) {
}

int32_t NetworkManagerRs::create(SocketType type, uint8_t protocol, const SocketArgs &args,
                                 capsel_t *caps) {
    KIF::ExchangeArgs eargs;
    ExchangeOStream os(eargs);
    os << Operation::CREATE
       << static_cast<uint64_t>(type) << protocol
       << args.rbuf_size << args.rbuf_slots
       << args.sbuf_size << args.sbuf_slots;
    eargs.bytes = os.total();
    KIF::CapRngDesc crd = obtain(2, &eargs);
    *caps = crd.start();

    int32_t sd;
    ExchangeIStream is(eargs);
    is >> sd;
    return sd;
}

void NetworkManagerRs::add_socket(SocketRs *socket) {
    _sockets.append(socket);
}

void NetworkManagerRs::remove_socket(SocketRs *socket) {
    _sockets.remove(socket);
}

IpAddr NetworkManagerRs::bind(int32_t sd, port_t port) {
    GateIStream reply = send_receive_vmsg(_metagate, BIND, sd, port);
    reply.pull_result();
    uint32_t addr;
    reply >> addr;
    return IpAddr(addr);
}

IpAddr NetworkManagerRs::listen(int32_t sd, port_t port) {
    GateIStream reply = send_receive_vmsg(_metagate, LISTEN, sd, port);
    reply.pull_result();
    uint32_t addr;
    reply >> addr;
    return IpAddr(addr);
}

port_t NetworkManagerRs::connect(int32_t sd, IpAddr remote_addr, port_t remote_port) {
    GateIStream reply = send_receive_vmsg(_metagate, CONNECT, sd, remote_addr.addr(), remote_port);
    reply.pull_result();
    port_t port;
    reply >> port;
    return port;
}

void NetworkManagerRs::abort(int32_t sd, bool remove) {
    GateIStream reply = send_receive_vmsg(_metagate, ABORT, sd, remove);
    reply.pull_result();
}

void NetworkManagerRs::wait(uint dirs) {
    while(true) {
        if(tick_sockets(dirs))
            break;

        VPE::sleep();
    }
}

void NetworkManagerRs::wait_for(uint64_t timeout, uint dirs) {
    uint64_t end = TCU::get().nanotime() + timeout;
    uint64_t now;
    while((now = TCU::get().nanotime()) < end) {
        if(tick_sockets(dirs))
            break;

        VPE::sleep_for(end - now);
    }
}

bool NetworkManagerRs::tick_sockets(uint dirs) {
    bool found = false;
    for(auto sock = _sockets.begin(); sock != _sockets.end(); ++sock) {
        sock->fetch_replies();
        if(((dirs & Direction::INPUT) && sock->process_events()) ||
            ((dirs & Direction::OUTPUT) && sock->can_send())) {
            found = true;
        }
    }
    return found;
}

} // namespace m3
