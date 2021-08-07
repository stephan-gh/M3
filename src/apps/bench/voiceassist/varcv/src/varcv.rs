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

use core::cmp;

use m3::col::Vec;
use m3::env;
use m3::net::{Port, StreamSocketArgs, TcpSocket};
use m3::println;
use m3::session::NetworkManager;

const REPEATS: u32 = 16;

fn usage(name: &str) -> ! {
    println!("Usage: {} <port>", name);
    m3::exit(1);
}

#[no_mangle]
pub fn main() -> i32 {
    let args = env::args().collect::<Vec<&str>>();
    if args.len() != 2 {
        usage(args[0]);
    }

    let port = args[1].parse::<Port>().unwrap_or_else(|_| usage(args[0]));

    let nm = NetworkManager::new("net").expect("connecting to net failed");

    let mut tcp_socket = TcpSocket::new(
        StreamSocketArgs::new(&nm)
            .send_buffer(64 * 1024)
            .recv_buffer(256 * 1024),
    )
    .expect("creating TCP socket failed");

    let mut buffer = [0u8; 1024];

    tcp_socket.listen(port).expect("listen failed");
    let ep = tcp_socket.accept().expect("accept failed");
    println!("Accepted remote endpoint {}", ep);

    for _ in 0..REPEATS {
        let mut length_bytes = [0u8; 8];
        let recv_len = tcp_socket
            .recv(&mut length_bytes)
            .expect("receive length failed");
        assert_eq!(recv_len, 8);

        let length = u64::from_le_bytes(length_bytes);
        println!("Expecting {} bytes", length);

        let mut rem = length;
        while rem > 0 {
            let amount = cmp::min(rem as usize, buffer.len());
            let recv_len = tcp_socket
                .recv(&mut buffer[0..amount])
                .expect("receive failed");
            println!("Received {} -> {}/{} bytes", recv_len, length - rem, length);
            rem -= recv_len as u64;
        }

        tcp_socket.send(&[0u8]).expect("send ACK failed");
    }

    tcp_socket.close().expect("close failed");

    0
}
