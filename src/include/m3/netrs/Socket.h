#pragma once

#include <base/col/List.h>
#include <base/col/Treap.h>

#include <m3/netrs/Net.h>
#include <m3/netrs/NetChannel.h>
#include <m3/session/NetworkManagerRs.h>

namespace m3 {

class SocketRs {


public:
    explicit SocketRs(SocketType ty, NetworkManagerRs &nm, uint8_t protocol);

    //Socket descriptor on the server
    int32_t _sd;
    IpAddr _local_addr;
    uint16_t _local_port;
    IpAddr _remote_addr;
    uint16_t _remote_port;

    //Reference to the network manager
    NetworkManagerRs &_nm;
};

}
