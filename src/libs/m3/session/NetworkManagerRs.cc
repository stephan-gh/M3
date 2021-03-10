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
#include <stdlib.h> //needed for mallocing list elements and received packages
#include <m3/com/GateStream.h>
#include <m3/session/NetworkManagerRs.h>
#include <m3/stream/Standard.h>
#include <m3/Exception.h>
#include <m3/netrs/NetChannel.h>
#include <thread/ThreadManager.h>

namespace m3 {

NetworkManagerRs::NetworkManagerRs(const String &service)
    : ClientSession(service),
      _metagate(SendGate::bind(obtain(1).start())),
      _channel(NetChannel(obtain(3).start())),
      _receive_queue(){
}


NetworkManagerRs::~NetworkManagerRs() {
    //Delete all packages that have not been taken yet
    RecvElement* queue_element;
    while((queue_element = _receive_queue.remove_root()) != nullptr){
	//Delete elements in this sockets queue
        queue_element->clear();
	//Now delete treap element
	delete queue_element;
    }
}

int32_t NetworkManagerRs::create(SocketType type, uint8_t protocol) {
    LLOG(NET, "Create:()");
    GateIStream reply = send_receive_vmsg(_metagate, CREATE, static_cast<uint64_t>(type), protocol);
    reply.pull_result();

    int32_t sd;
    reply >> sd;

    /*
    Socket *socket = Socket::new_socket(type, sd, *this);
    socket->_channel = _channel;
    _sockets.insert(socket);
    */
    return sd;
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

void NetworkManagerRs::connect(int32_t sd, IpAddr remote_addr, uint16_t remote_port, IpAddr local_addr, uint16_t local_port) {
    LLOG(NET, "Connect:()");
    GateIStream reply = send_receive_vmsg(_metagate, CONNECT, sd, remote_addr.addr(), remote_port, local_addr.addr(), local_port);
    reply.pull_result();
}

void NetworkManagerRs::close(int32_t sd) {
    GateIStream reply = send_receive_vmsg(_metagate, CLOSE, sd);
    reply.pull_result();
}

void m3::NetworkManagerRs::as_file(int sd, int mode, MemGate& mem, size_t memsize, fd_t& fd) {
    LLOG(NET, "Warning: as_file is unimplemented!");
    throw Exception(Errors::NOT_SUP);;
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

void NetworkManagerRs::notify_drop(int32_t sd){
    close(sd);
}
    
void NetworkManagerRs::send(int32_t sd, IpAddr src_addr, uint16_t src_port, IpAddr dst_addr, uint16_t dst_port, uint8_t *data, uint32_t data_length){
    //Wrap our data into a NetData struct and send it
    LLOG(NET, "Send:(sd=" << sd << ", size=" << data_length << ")");
    
    m3::net::NetData wrapped_data = m3::net::NetData(sd, data, data_length, src_addr, src_port, dst_addr, dst_port);
    _channel.send(wrapped_data);
    //Tick server
    GateIStream reply = send_receive_vmsg(_metagate, TICK, sd);
    reply.pull_result();
}

m3::net::NetData NetworkManagerRs::recv(int32_t sd){
    update_recv_queue();

    //Now check if we can take out a package
    RecvElement* el = _receive_queue.find(sd);
    if (el != nullptr){
        //Try to get a package out of the queue, if there is non return.
	m3::net::NetData* package = el->pop_element();
	if (package == nullptr){
	    return m3::net::NetData();
	}
	//Copy package into stack allocated value
	m3::net::NetData new_pkg = m3::net::NetData();
	memcpy((void*)&new_pkg, (void*)package, sizeof(m3::net::NetData));

	//Delete allocation
	delete package;
	
	LLOG(NET, "Recved: ");
	new_pkg.log();
	return new_pkg;
    }else{
	return m3::net::NetData();
    }
}
SocketState NetworkManagerRs::get_state(int32_t sd){
    GateIStream reply = send_receive_vmsg(_metagate, QUERY_STATE, sd);
    reply.pull_result();
    SocketState state;
    uint64_t socket_type;
    uint64_t socket_state;
    reply >> socket_type >> socket_state;

    LLOG(NET, "NetworkManger::get_state(): SocketType=" << socket_type << ", State=" << socket_state);
    state._socket_type = socket_type;
    state._socket_state = socket_state;

    
    return state;
}
void NetworkManagerRs::update_recv_queue(){
    //Pull packages from the channel and store them until no packages are received anymore
    while(1){
	m3::net::NetData* pkg = _channel.receive();
	if (pkg == nullptr){
	    break;
	}else{
	    //Got a valid package, either insert it into the already pending queue, or create a queue for this packages's
	    //sd.
	    RecvElement* sd_queue = _receive_queue.find(pkg->sd);
	    if (sd_queue == nullptr){
	        //No queue for this sd yet, therefore create one
		RecvElement* el = new RecvElement(pkg);
	        //insert this sockets queue into treap
		_receive_queue.insert(el);
	    }else{
		//there is a queue for this descriptor, therefore just push
		sd_queue->push(pkg);
	    }
	}
    }
}
}  // namespace m3
