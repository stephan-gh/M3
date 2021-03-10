#include <m3/netrs/NetChannel.h>
#include <m3/netrs/Net.h>
#include <base/log/Lib.h>

namespace m3{
    NetChannel::NetChannel(capsel_t caps)
	: _rg(RecvGate::bind(caps + 0, nextlog2<MSG_BUF_SIZE>::val, nextlog2<MSG_SIZE>::val)),
	  _sg(SendGate::bind(caps + 1, nullptr)),
	  _mem(MemGate::bind(caps + 2))
    {
	//Activate the rgate manually
	_rg.activate();
    }
    
    void NetChannel::send(m3::net::NetData data){
	LLOG(NET, "NetLogSend:");
	data.log();
	_sg.send(&data, sizeof(m3::net::NetData));
    }
    
    m3::net::NetData* NetChannel::receive(){
        const TCU::Message* msg = _rg.fetch();
	if (msg != nullptr){
	    LLOG(NET, "msglength=" << msg->length << " sizeof=" << sizeof(m3::net::NetData));
	    //this is an actual package, therefore copy the data into a buffer thats cast
	    // into the NetData struct
	    m3::net::NetData* package = new m3::net::NetData();
	    //TODO Somehow prevent copy?
	    memcpy(static_cast<void*>(package), msg->data, sizeof(m3::net::NetData));
	    //package->log();
	    //Ack message to free channel
	    _rg.ack_msg(msg);
	    return package;
	}else{
	    return nullptr;
	}
    }
}
    
