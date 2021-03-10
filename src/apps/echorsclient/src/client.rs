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
    let msg: &[u8; 10] = b"HiServer__";

    println!("CLIENT: Create manager");
    let manager = NetworkManager::new("net0").expect("Failed to create Network manager!");

    println!("CLIENT: Create socket");
    let mut socket = TcpSocket::new(&manager).unwrap();
    println!("CLIENT: Socket setup finished");

    socket.set_blocking(true);

    Semaphore::attach("net")
        .expect("Failed to get semaphore")
        .down()
        .expect("Failed to down sem");

    // socket.bind(IpAddr::new(127, 0, 0, 1), 1234).unwrap();

    // Wait for server to allow connection
    socket
        .connect(
            IpAddr::new(127, 0, 0, 2),
            1234, // remote
            IpAddr::new(0, 0, 0, 0),
            65000, // local
        )
        .expect("Failed to connect in client");

    assert!(
        socket.state().unwrap() == TcpState::Established,
        "Socket state did not match"
    );

    for _ in 0..10 {
        if let Err(e) = socket.send(msg.as_ref()) {
            println!("Failed to send client data: {}", e);
        }

        let package = socket.recv().expect("Failed to receive package");
        println!(
            "Client got answer: {}, from: {}",
            core::str::from_utf8(package.raw_data()).unwrap(),
            package.source_addr
        );
    }

    socket.close().expect("Failed to close client socket");
    println!("Client end gracefully");

    0
}
