/*
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

#include <m3/com/GateStream.h>
#include <m3/session/NetworkManager.h>
#include <m3/stream/Standard.h>
#include <m3/pes/VPE.h>
#include <m3/Exception.h>

#include <thread/ThreadManager.h>

namespace m3 {

NetworkManager::NetworkManager(const String &service)
    : ClientSession(service),
      _wloop(),
      _metagate(SendGate::bind(obtain(1).start())),
      _waiting_credit(0) {
}

NetworkManager::NetworkManager(capsel_t session, capsel_t metagate)
    : ClientSession(session),
      _wloop(),
      _metagate(SendGate::bind(metagate)),
      _waiting_credit(0) {
}

m3::NetworkManager::~NetworkManager() {
    Socket *socket;
    while((socket = _sockets.remove_root()) != nullptr)
        delete socket;
}

Socket* NetworkManager::create(Socket::SocketType type, uint8_t protocol) {
    ensure_channel_established();

    GateIStream reply = send_receive_vmsg(_metagate, CREATE, type, protocol);
    reply.pull_result();

    int sd;
    reply >> sd;

    Socket *socket = Socket::new_socket(type, sd, *this);
    socket->_channel = _channel;
    _sockets.insert(socket);

    return socket;
}

void NetworkManager::bind(int sd, IpAddr addr, uint16_t port) {
    GateIStream reply = send_receive_vmsg(_metagate, BIND, sd, addr.addr(), port);
    reply.pull_result();
}

void NetworkManager::listen(int sd) {
    GateIStream reply = send_receive_vmsg(_metagate, LISTEN, sd);
    reply.pull_result();
}

void NetworkManager::connect(int sd, IpAddr addr, uint16_t port) {
    GateIStream reply = send_receive_vmsg(_metagate, CONNECT, sd, addr.addr(), port);
    reply.pull_result();
}

void NetworkManager::close(int sd) {
    GateIStream reply = send_receive_vmsg(_metagate, CLOSE, sd);
    reply.pull_result();
}

void m3::NetworkManager::as_file(int sd, int mode, MemGate& mem, size_t memsize, fd_t& fd) {
    // Create file session for socket
    KIF::ExchangeArgs args;
    ExchangeOStream os(args);
    os << sd << mode << (mode & FILE_R ? memsize : 0) << (mode & FILE_W ? memsize : 0);
    args.bytes = os.total();
    KIF::CapRngDesc desc = obtain(2, &args);

    // Delegate shared memory to file session
    ClientSession fs(desc.start());
    KIF::CapRngDesc shm_crd(KIF::CapRngDesc::OBJ, mem.sel(), 1);

    ExchangeOStream shm_os(args);
    shm_os << sd;
    args.bytes = shm_os.total();
    fs.delegate(shm_crd, &args);

    fd = VPE::self().fds()->alloc(Reference<File>(new GenericFile(mode, desc.start())));
}


void NetworkManager::ensure_channel_established() {
    // Channel already established
    if(_channel)
        return;

    // Obtain channel
    KIF::CapRngDesc caps = obtain(3);
    _channel = Reference<NetEventChannel>(new NetEventChannel(caps.start(), false));
}

void NetworkManager::listen_channel(NetEventChannel& _channel) {
    assert(_wloop);
    using namespace std::placeholders;
    _channel.start(_wloop, std::bind(&NetworkManager::process_event, this, _1),
            std::bind(&NetworkManager::process_credit, this, _1, _2));
}

void NetworkManager::wait_for_credit(NetEventChannel& _channel) {
    _waiting_credit++;

    if(_channel.get_credit_event() == 0)
        _channel.set_credit_event(ThreadManager::get().get_wait_event());
    _channel.wait_for_credit();
}

void NetworkManager::wait_sync() {
    using namespace std::placeholders;
    NetEventChannel::evhandler_t ev = std::bind(&NetworkManager::process_event, this, _1);
    NetEventChannel::crdhandler_t crd;

    while(1) {
        // This would be the place to implement timeouts.
        VPE::sleep();

        if(_channel->has_events(ev, crd))
            break;
    }
}

Socket *NetworkManager::process_event(NetEventChannel::Event &event) {
    if(!event.is_present())
        return nullptr;

    auto message = static_cast<NetEventChannel::SocketControlMessage const *>(event.get_message());
    LLOG(NET, "NetworkManager::process_event: type=" << message->type << ", sd=" << message->sd);

    Socket * socket = _sockets.find(message->sd);
    if(!socket) {
        LLOG(NET, "Received event with invalid socket descriptor: " << message->sd);
        return nullptr;
    }

    // TODO socket leaks if this throws
    socket->process_message(*message, event);
    return socket;
}

void NetworkManager::process_credit(event_t wait_event, size_t waiting) {
    LLOG(NET, "NetworkManager::process_credit: wait_event=" << wait_event << ", waiting=" << waiting);
    _waiting_credit -= waiting;
    ThreadManager::get().notify(wait_event);
}

}  // namespace m3
