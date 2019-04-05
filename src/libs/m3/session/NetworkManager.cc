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

#include <thread/ThreadManager.h>

namespace m3 {

NetworkManager::NetworkManager(const String &service)
    : ClientSession(service), _metagate(SendGate::bind(obtain(1).start())),
      _waiting_credit(0) {
    env()->workloop()->set_sleep_handler(std::bind(&NetworkManager::process_sleep, this));
}

NetworkManager::NetworkManager(capsel_t session, capsel_t metagate)
    : ClientSession(session), _metagate(SendGate::bind(metagate)),
      _waiting_credit(0) {
    env()->workloop()->set_sleep_handler(std::bind(&NetworkManager::process_sleep, this));
}

m3::NetworkManager::~NetworkManager() {
    Socket *socket;
    while((socket = _sockets.remove_root()) != nullptr) {
        delete socket;
    }
    env()->workloop()->set_sleep_handler(nullptr);
}

Socket* NetworkManager::create(Socket::SocketType type, uint8_t protocol) {
    Errors::last = ensure_channel_established();
    if(Errors::last != Errors::NONE)
        return nullptr;

    GateIStream reply = send_receive_vmsg(_metagate, CREATE, type, protocol);
    reply >> Errors::last;
    if(Errors::last == Errors::NONE) {
        int sd;
        reply >> sd;

        Socket *socket = Socket::new_socket(type, sd, *this);
        socket->_channel = _channel;

        _sockets.insert(socket);

        return socket;
    }
    return nullptr;
}

Errors::Code NetworkManager::bind(int sd, IpAddr addr, uint16_t port) {
    GateIStream reply = send_receive_vmsg(_metagate, BIND, sd, addr.addr(), port);
    reply >> Errors::last;
    return Errors::last;
}

Errors::Code NetworkManager::listen(int sd) {
    GateIStream reply = send_receive_vmsg(_metagate, LISTEN, sd);
    reply >> Errors::last;
    return Errors::last;
}

Errors::Code NetworkManager::connect(int sd, IpAddr addr, uint16_t port) {
    GateIStream reply = send_receive_vmsg(_metagate, CONNECT, sd, addr.addr(), port);
    reply >> Errors::last;
    return Errors::last;
}

Errors::Code NetworkManager::close(int sd) {
    GateIStream reply = send_receive_vmsg(_metagate, CLOSE, sd);
    reply >> Errors::last;
    return Errors::last;
}

Errors::Code m3::NetworkManager::as_file(int sd, int mode, MemGate& mem, size_t memsize, fd_t& fd) {
    // Create file session for socket
    KIF::ExchangeArgs fs_args;
    fs_args.count = 4;
    fs_args.vals[0] = static_cast<xfer_t>(sd);
    fs_args.vals[1] = static_cast<xfer_t>(mode);
    fs_args.vals[2] = mode & FILE_R ? memsize : 0;
    fs_args.vals[3] = mode & FILE_W ? memsize : 0;
    KIF::CapRngDesc desc = obtain(2, &fs_args);
    if(Errors::last != Errors::NONE)
        return Errors::last;

    // Delegate shared memory to file session
    ClientSession fs(desc.start());
    KIF::CapRngDesc shm_crd(KIF::CapRngDesc::OBJ, mem.sel(), 1);
    KIF::ExchangeArgs shm_args;
    shm_args.count = 1;
    shm_args.vals[0] = static_cast<xfer_t>(sd);
    if(fs.delegate(shm_crd, &shm_args) != Errors::NONE)
        return Errors::last;

    fd = VPE::self().fds()->alloc(Reference<File>(new GenericFile(mode, desc.start())));
    return Errors::NONE;
}


Errors::Code NetworkManager::ensure_channel_established() {
    // Channel already established
    if(_channel.valid())
        return Errors::NONE;

    // Obtain channel
    KIF::ExchangeArgs args;
    args.count = 0;
    KIF::CapRngDesc caps = obtain(3, &args);
    if(Errors::last != Errors::NONE)
        return Errors::last;

    _channel = Reference<NetEventChannel>(new NetEventChannel(caps.start(), false));
    return Errors::NONE;
}

void NetworkManager::listen_channel(NetEventChannel& _channel) {
    using namespace std::placeholders;
    _channel.start(std::bind(&NetworkManager::process_event, this, _1),
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
        process_sleep();
        if(_channel->has_events(ev, crd))
            break;
    }
}

Socket * NetworkManager::process_event(NetEventChannel::Event &event) {
    if(!event.is_present())
        return nullptr;

    auto message = static_cast<NetEventChannel::SocketControlMessage const *>(event.get_message());
    LLOG(NET, "NetworkManager::process_event: type=" << message->type << ", sd=" << message->sd);

    Socket * socket = _sockets.find(message->sd);
    if(!socket) {
        LLOG(NET, "Received event with invalid socket descriptor: " << message->sd);
        return nullptr;
    }

    auto result = socket->process_message(*message, event);
    if(result != Errors::NONE) {
        LLOG(NET, "Processing of message " << message->type << " by socket " << message->sd << " failed.");
    }
    return socket;
}

void NetworkManager::process_credit(event_t wait_event, size_t waiting) {
    LLOG(NET, "NetworkManager::process_credit: wait_event=" << wait_event << ", waiting=" << waiting);
    _waiting_credit -= waiting;
    ThreadManager::get().notify(wait_event);
}

void NetworkManager::process_sleep() {
#if defined(__gem5__)
    DTU::reg_t event_mask = DTU::EventMask::MSG_RECV;
    if(_waiting_credit > 0)
        event_mask |= DTU::EventMask::CRD_RECV;

    if(!(DTU::get().fetch_events() & event_mask)) {
        LLOG(NET, "NetworkManager::process_sleep: Trying to sleep!");
        // This would be the place to implement timeouts.
        DTU::get().try_sleep(true, 0, event_mask);
    }
#endif
}

}  // namespace m3
