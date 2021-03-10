#include <m3/netrs/Socket.h>
#include <m3/netrs/UdpSocket.h>
#include <m3/session/NetworkManagerRs.h>
#include <m3/Exception.h>

namespace m3{    
    UdpSocketRs::UdpSocketRs(NetworkManagerRs& nm)
	: _socket(SocketType::SOCK_DGRAM, nm, 0){
    }
    void UdpSocketRs::bind(IpAddr addr, uint16_t port){
	_socket._nm.bind(_socket._sd, addr, port);
    }
    m3::net::NetData UdpSocketRs::recv(){
	if (_is_blocking){
	    //Wait until we get a non empty package
	    while (true){
		m3::net::NetData pkg = _socket._nm.recv(_socket._sd);
		if (!pkg.is_empty()){
		    return pkg;
		}
	    }
	}else{
	    return _socket._nm.recv(_socket._sd);
	}
    }
    void UdpSocketRs::send(IpAddr dest_addr, uint16_t dest_port, uint8_t* data, uint32_t size){
	//Specify destination address, source will be filled in by the service
	_socket._nm.send(_socket._sd, IpAddr(), 0, dest_addr, dest_port, data, size);
    }
    UdpState UdpSocketRs::state(){
	SocketState state = _socket._nm.get_state(_socket._sd);
	return state.udp_state();
    }

    void UdpSocketRs::set_blocking(bool should_block){
	_is_blocking = should_block;
    }
}
