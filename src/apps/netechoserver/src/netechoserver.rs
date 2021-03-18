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

use m3::com::Semaphore;
use m3::net::{IpAddr, State, TcpSocket, UdpSocket};
use m3::session::NetworkManager;

#[no_mangle]
pub fn main() -> i32 {
    let nm = NetworkManager::new("net1").expect("connecting to net1 failed");

    let mut udp_socket = UdpSocket::new(&nm).expect("creating UDP socket failed");
    let mut tcp_socket = TcpSocket::new(&nm).expect("creating TCP socket failed");

    udp_socket
        .bind(IpAddr::new(192, 168, 112, 1), 1337)
        .expect("bind failed");

    Semaphore::attach("net-udp")
        .expect("attaching to net-udp semaphore failed")
        .up()
        .expect("udp up failed");

    let sem_tcp = Semaphore::attach("net-tcp").expect("attaching to net-tcp semaphore failed");

    let mut buffer = [0u8; 1024];

    loop {
        if tcp_socket.state() == State::Closed {
            tcp_socket
                .listen(IpAddr::new(192, 168, 112, 1), 1338)
                .expect("listen failed");
            sem_tcp.up().expect("tcp up failed");
        }

        if udp_socket.has_data() {
            let (size, ip, port) = udp_socket.recv_from(&mut buffer).unwrap();
            udp_socket.send_to(&buffer[0..size], ip, port).unwrap();
        }

        if tcp_socket.has_data() {
            let (size, ip, port) = tcp_socket.recv_from(&mut buffer).unwrap();
            tcp_socket.send_to(&buffer[0..size], ip, port).unwrap();
        }

        if !udp_socket.has_data() && !tcp_socket.has_data() {
            if tcp_socket.state() == State::Closing {
                tcp_socket.abort().unwrap();
            }
            else {
                nm.wait_sync();
            }
        }

        nm.process_events(None);
    }
}
