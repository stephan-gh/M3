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

use m3::net::{Endpoint, IpAddr, StreamSocketArgs, TcpSocket};
use m3::session::{NetworkManager};

#[no_mangle]
pub fn main() -> i32 {
    let nm = NetworkManager::new("net").expect("connecting to net failed");

    let buffer = [0u8; 1024];

    let mut tcp_socket = TcpSocket::new(StreamSocketArgs::new(&nm)).expect("creating TCP socket failed");
    tcp_socket.connect(Endpoint::new(IpAddr::new(192, 168, 42, 15), 22)).expect("connect failed");
    tcp_socket.send(&buffer[..]).expect("send failed");

    0
}
