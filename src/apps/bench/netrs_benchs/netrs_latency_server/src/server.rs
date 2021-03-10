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

use m3::{
    com::Semaphore,
    net::{IpAddr, UdpSocket},
    session::NetworkManager,
};

#[no_mangle]
pub fn main() -> i32 {
    let nm = NetworkManager::new("net1").expect("connecting to net1 failed");
    let mut socket = UdpSocket::new(&nm).expect("creating UDP socket failed");
    socket.set_blocking(true);

    socket
        .bind(IpAddr::new(192, 168, 112, 1), 1337)
        .expect("bind failed");

    Semaphore::attach("net")
        .expect("attaching to net semaphore failed")
        .up()
        .expect("notifying net semaphore failed");

    loop {
        let pkg = socket.recv().unwrap();
        socket
            .send(pkg.source_addr, pkg.source_port, pkg.raw_data())
            .expect("send failed");
    }
}
