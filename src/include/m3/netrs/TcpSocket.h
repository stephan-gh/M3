#pragma once

#include <m3/netrs/Socket.h>
#include <m3/session/NetworkManagerRs.h>

namespace m3 {

class TcpSocketRs{
public:    
    explicit TcpSocketRs(NetworkManagerRs& nm); 
    ~TcpSocketRs();
    
    void set_blocking(bool should_block);
    void listen(IpAddr addr, uint16_t port);
    void connect(IpAddr remote_addr, uint16_t remote_port, IpAddr local_addr, uint16_t local_port);
    /**
     *Returns a net data package. It the socket is not in blocking mode, the package might be empty.
     */
    m3::net::NetData recv();
    void send(uint8_t* data, uint32_t size);
    TcpState state();
    void close();
private:
    void wait_for_state(TcpState target_state);
    
    bool _blocking;
    bool _is_closed;
    SocketRs _socket;
};
}
