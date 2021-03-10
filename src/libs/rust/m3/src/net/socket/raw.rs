use crate::col::Vec;
use crate::errors::Error;
use crate::net::{socket::Socket, SocketType};
use crate::session::NetworkManager;

///A Raw socket sends already finished packages. Therefore the IpHeader must be written, before the package is passed to send.
pub struct RawSocket<'a> {
    #[allow(dead_code)]
    socket: Socket<'a>,
}

impl<'a> RawSocket<'a> {
    pub fn new(
	network_manager: &'a NetworkManager,
	protocol: Option<u8>,
	
    ) -> Result<Self, Error> {
        Ok(RawSocket {
            socket: Socket::new(SocketType::Raw, network_manager, protocol)?,
        })
    }

    pub fn send(_data: &[u8]) -> Result<usize, Error> {
        Ok(0)
    }

    pub fn recv_msg<T>(&self) -> Result<Vec<T>, Error> {
        Ok(Vec::new())
    }
}
