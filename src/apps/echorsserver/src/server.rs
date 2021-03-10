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
use m3::net::{TcpSocket, TcpState};
use m3::session::NetworkManager;

#[no_mangle]
pub fn main() -> i32 {
    println!("SERVER: Create network manager");
    let net = NetworkManager::new("net1").unwrap();
    println!("SERVER: Create socket");
    let mut socket = TcpSocket::new(&net).unwrap();

    socket.set_blocking(true);
    println!("SERVER: listen");
    socket.listen(IpAddr::new(127, 0, 0, 2), 1234).unwrap();

    assert!(
        socket.state().unwrap() == TcpState::Listen,
        "Socket state did not match"
    );

    // Signal that we are listening
    Semaphore::attach("net")
        .expect("Failed to get semaphore")
        .up()
        .expect("Failed to up sem");

    let msg: &[u8; 7] = b"HiBack!";
    for _ in 0..10 {
        let package = socket.recv().expect("Failed to receive package!");
        println!(
            "SERVER: Received {}\nfrom: {}",
            core::str::from_utf8(&package.raw_data()).unwrap(),
            package.source_addr
        );
        socket.send(msg.as_ref()).expect("Failed to send on server");
    }

    socket.close().expect("Failed to close server socket");
    println!("Server end gracefully");
    0
}
