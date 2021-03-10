
#include <m3/netrs/Net.h>
#include <m3/session/NetworkManagerRs.h>
#include <m3/netrs/Socket.h>
#include <base/log/Lib.h>
namespace m3{
SocketRs::SocketRs(SocketType ty, NetworkManagerRs &nm, uint8_t protocol)
    :_nm(nm){
    int32_t sd = nm.create(ty, protocol);
    if (sd < 0){
	LLOG(NET, "Failed to create socket: Could not allocate socket descriptor!");
	//TODO other error
	throw Exception(Errors::NOT_SUP);
    }

    _sd = sd;
    //Init other parameters that might be set while using this socket.
    _local_addr = IpAddr(0,0,0,0);
    _local_port = 0;
    _remote_addr = IpAddr(0,0,0,0);
    _remote_port = 0;
}

}
