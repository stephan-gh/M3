/*
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * This file is part of M3 (Microkernel-based SysteM for Heterogeneous Manycores).
 *
 * M3 is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License version 2 as
 * published by the Free Software Foundation.
 *
 * M3 is distributed in the hope that it will be useful, but
 * WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * General Public License version 2 for more details.
 */

#![no_std]

#[macro_use]
extern crate m3;

use m3::com::Semaphore;
use m3::net::IpAddr;
use m3::net::TcpSocket;
use m3::session::NetworkManager;

#[no_mangle]
pub fn main() -> i32 {
    println!("SERVER: Create network manager");
    let net = NetworkManager::new("net1").unwrap();
    println!("SERVER: Create socket");
    let mut socket = TcpSocket::new(&net).unwrap();

    println!("SERVER: listen");
    socket.listen(IpAddr::new(127, 0, 0, 2), 1234).unwrap();

    // Signal that we are listening
    Semaphore::attach("net")
        .expect("Failed to get semaphore")
        .up()
        .expect("Failed to up sem");

    println!("SERVER: accept");
    let (ip, port) = socket.accept().unwrap();
    println!("SERVER: connected to {}:{}", ip, port);

    println!("SERVER: starting loop");

    let req: &[u8; 7] = b"HiBack!";
    let mut resp = [0u8; 1024];
    for _ in 0..10 {
        let (size, ip, port) = socket.recv_from(&mut resp).expect("Failed to receive package!");
        println!(
            "SERVER: Received '{}' from: {}:{}",
            core::str::from_utf8(&resp[0..size]).unwrap(),
            ip, port
        );
        socket.send(req).expect("Failed to send on server");
    }

    socket.close().expect("Failed to close server socket");
    println!("Server end gracefully");
    0
}
