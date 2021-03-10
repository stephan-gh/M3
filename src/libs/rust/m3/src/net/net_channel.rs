use base::llog;

use crate::com::{RecvGate, SendGate, MemGate};
use crate::cap::Selector;
use crate::errors::{Error, Code};
use crate::net::NetData;
use crate::tcu::TCUIf;

use super::{MSG_BUF_ORDER, MSG_ORDER};

pub struct NetChannel{
    sg: SendGate,
    rg: RecvGate,
    
    #[allow(dead_code)]
    mem: MemGate //TODO Used when socket as file is used?
}

impl NetChannel{
    ///Creates a new channel that is bound to `caps` and `caps+2`. Assumes that the `caps` where obtained from the netrs service, and are valid gates
    pub fn new_with_gates(send: SendGate, mut recv: RecvGate, mem: MemGate) -> Self{
	//activate rgate
	recv.activate().expect("Failed to activate server rgate");
	
	NetChannel{
	    sg: send,
	    rg: recv,
	    mem
	}
    }

    ///Does not crate new gates for this channel, but binds to them at `caps`-`caps+2`
    pub fn bind(caps: Selector) -> Result<Self, Error>{
	let mut rgate = RecvGate::new_bind(caps + 0, MSG_BUF_ORDER, MSG_ORDER);
	rgate.activate().expect("Failed to activate rgate");
	let sgate = SendGate::new_bind(caps + 1);
	let mgate = MemGate::new_bind(caps + 2);

	Ok(NetChannel{
	    sg: sgate,
	    rg: rgate,
	    mem: mgate
	})
    }

    ///Sends data over the send gate
    pub fn send(&self, net_data: NetData) -> Result<(), Error>{
	self.sg.send(&[net_data], &self.rg)?;
	Ok(())
    }

    ///Tries to receive a message from the other side
    pub fn receive(&self) -> Result<NetData, Error>{
	//Fetch message by hand, if something is fetched,
	//assumes that it is a NetData package.
	if let Some(msg) = TCUIf::fetch_msg(&self.rg){
	    //TODO can we get around the clone?
	    let net_data = msg.get_data::<NetData>().clone();
	    //mark message as read
	    self.rg.ack_msg(msg)?;
	    
	    Ok(net_data)
	}else{
	    Err(Error::new(Code::NotSup))
	}
    }
}
