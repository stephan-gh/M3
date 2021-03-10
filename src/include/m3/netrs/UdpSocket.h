#pragma once

#include <m3/netrs/Socket.h>
#include <m3/session/NetworkManagerRs.h>

namespace m3 {
class UdpSocketRs{
public:    
    explicit UdpSocketRs(NetworkManagerRs& nm);
    void bind(IpAddr addr, uint16_t port);
    /**
     *Returns a net data package. It the socket is not in blocking mode, the package might be empty.
     */
    m3::net::NetData recv();
    void send(IpAddr dest_addr, uint16_t dest_port, uint8_t* data, uint32_t size);
    UdpState state();
    void set_blocking(bool should_block);
private:
    bool _is_blocking;
    SocketRs _socket;
};
}
