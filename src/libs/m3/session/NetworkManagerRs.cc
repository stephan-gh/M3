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

#include <stdlib.h> // needed for mallocing list elements and received packages
#include <thread/ThreadManager.h>

namespace m3 {

NetworkManagerRs::NetworkManagerRs(const String &service)
    : ClientSession(service),
      _metagate(SendGate::bind(obtain(1).start())),
      _channel(NetEventChannelRs(obtain(3).start())) {
}

int32_t NetworkManagerRs::create(SocketType type, uint8_t protocol) {
    LLOG(NET, "Create:()");
    GateIStream reply = send_receive_vmsg(_metagate, CREATE, static_cast<uint64_t>(type), protocol);
    reply.pull_result();

    int32_t sd;
    reply >> sd;
    return sd;
}

void NetworkManagerRs::add_socket(SocketRs *socket) {
    _sockets.insert(socket);
}

void NetworkManagerRs::remove_socket(SocketRs *socket) {
    _sockets.remove(socket);
}

void NetworkManagerRs::bind(int32_t sd, IpAddr addr, uint16_t port) {
    LLOG(NET, "Bind:()");
    GateIStream reply = send_receive_vmsg(_metagate, BIND, sd, addr.addr(), port);
    reply.pull_result();
}

void NetworkManagerRs::listen(int32_t sd, IpAddr local_addr, uint16_t port) {
    LLOG(NET, "Listen:()");
    GateIStream reply = send_receive_vmsg(_metagate, LISTEN, sd, local_addr.addr(), port);
    reply.pull_result();
}

void NetworkManagerRs::connect(int32_t sd, IpAddr remote_addr, uint16_t remote_port, uint16_t local_port) {
    LLOG(NET, "Connect:()");
    GateIStream reply = send_receive_vmsg(_metagate, CONNECT,
                                          sd, remote_addr.addr(), remote_port, local_port);
    reply.pull_result();
}

bool NetworkManagerRs::close(int32_t sd) {
    return _channel.send_close_req(sd);
}

void NetworkManagerRs::abort(int32_t sd, bool remove) {
    LLOG(NET, "Abort:()");
    GateIStream reply = send_receive_vmsg(_metagate, ABORT, sd, remove);
    reply.pull_result();
}

void m3::NetworkManagerRs::as_file(int sd, int mode, MemGate &mem, size_t memsize, fd_t &fd) {
    LLOG(NET, "Warning: as_file is unimplemented!");
    throw Exception(Errors::NOT_SUP);
    ;
    /*
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
    */
}

ssize_t NetworkManagerRs::send(int32_t sd, IpAddr dst_addr, uint16_t dst_port,
                               const void *data, size_t data_length) {
    LLOG(NET, "Send:(sd=" << sd << ", size=" << data_length << ")");
    bool succeeded = _channel.send_data(sd, dst_addr, dst_port, data_length, [data, data_length](void *buf) {
        memcpy(buf, data, data_length);
    });
    if(!succeeded)
        return -1;
    return static_cast<ssize_t>(data_length);
}

SocketRs *NetworkManagerRs::process_event(NetEventChannelRs::Event &event) {
    if(!event.is_present())
        return nullptr;

    auto message = static_cast<NetEventChannelRs::SocketControlMessage const *>(event.get_message());
    LLOG(NET, "NetworkManager::process_event: type=" << message->type << ", sd=" << message->sd);

    SocketRs *socket = _sockets.find(message->sd);
    if(!socket) {
        LLOG(NET, "Received event with invalid socket descriptor: " << message->sd);
        return nullptr;
    }

    // TODO socket leaks if this throws
    socket->process_message(*message, event);
    return socket;
}

void NetworkManagerRs::wait_sync() {
    while(1) {
        // This would be the place to implement timeouts.
        VPE::sleep();

        if(_channel.has_events())
            break;
    }
}

NetEventChannelRs::Event NetworkManagerRs::recv_event() {
    return _channel.recv_message();
}

} // namespace m3
