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

use m3::{
    com::Semaphore,
    net::{IpAddr, UdpSocket},
    println,
    session::NetworkManager,
};

#[no_mangle]
pub fn main() -> i32 {
    let nm = NetworkManager::new("net1").unwrap();
    let mut socket = UdpSocket::new(&nm).unwrap();
    socket.set_blocking(true);

    socket.bind(IpAddr::new(192, 168, 112, 1), 1337).unwrap();

    Semaphore::attach("net").unwrap().up();

    let request = [0 as u8; 1024];
    loop {
        let mut got_one = false;
        //Wait for at least one package before sending one back
        let _pkg = socket.recv().unwrap();
        socket.send(IpAddr::new(192, 168, 112, 2), 1337, &request);
    }

    0
}
