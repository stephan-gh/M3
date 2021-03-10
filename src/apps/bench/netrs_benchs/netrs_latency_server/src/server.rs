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

    loop{
	let pkg = socket.recv().unwrap();
	socket.send(pkg.source_addr, pkg.source_port, pkg.raw_data());
    }
    
    0
}
