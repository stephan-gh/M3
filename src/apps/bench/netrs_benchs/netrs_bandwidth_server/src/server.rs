#![no_std]

#[macro_use]
extern crate m3;

use m3::{com::Semaphore, net::{IpAddr, UdpSocket}, println, session::NetworkManager};

#[no_mangle]
pub fn main() -> i32 {

    let nm = NetworkManager::new("net1").unwrap();
    let mut socket = UdpSocket::new(&nm).unwrap();
    socket.set_blocking(true);

    socket.bind(IpAddr::new(192, 168, 112, 1), 1337).unwrap();

    Semaphore::attach("net").unwrap().up();

    let request = [0 as u8; 1024];
    loop{
	let mut got_one = false;
	//Wait for at least one package before sending one back
	let _pkg = socket.recv().unwrap();	    
	socket.send(IpAddr::new(192, 168, 112, 2), 1337, &request);
    }
    
    0
}
