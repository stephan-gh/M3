#pragma once

#include <base/TCU.h>
#include <base/util/Reference.h>

#include <m3/com/GateStream.h>
#include <m3/com/MemGate.h>
#include <m3/com/RecvGate.h>
#include <m3/com/SendGate.h>
#include <m3/netrs/Net.h>

namespace m3{
    
static const uint32_t MSG_SIZE = 2048;
static const uint32_t MSG_ORDER = 11;
static const uint32_t MSG_CREDITS = 4;
static const uint32_t MSG_CREDITS_ORDER = 2;
static const uint32_t MSG_BUF_SIZE = MSG_SIZE * MSG_CREDITS;
static const uint32_t MSG_BUF_ORDER = MSG_ORDER + MSG_CREDITS_ORDER;

class NetChannel{
public:
    ///Binds a channel to caps. Assumes a service is holding a RecvGate at caps+0, SendGate at caps+1 and MemGate at caps+2.
    explicit NetChannel(capsel_t caps);
    void send(m3::net::NetData data);
    //Tries to fetch a NetData package. If non exists an empty package is returned.
    m3::net::NetData* receive();
    
private:
    SendGate _sg;
    RecvGate _rg;
    MemGate _mem;
};
}
