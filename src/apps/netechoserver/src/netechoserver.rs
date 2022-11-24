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
use m3::errors::Error;
use m3::net::{
    DGramSocket, DgramSocketArgs, Socket, State, StreamSocket, StreamSocketArgs, TcpSocket,
    UdpSocket,
};
use m3::session::NetworkManager;
use m3::vfs::{FileEvent, FileWaiter};

#[no_mangle]
pub fn main() -> Result<(), Error> {
    let nm = NetworkManager::new("net").expect("connecting to net failed");

    let mut udp_socket = UdpSocket::new(
        DgramSocketArgs::new(nm.clone())
            .send_buffer(8, 64 * 1024)
            .recv_buffer(32, 256 * 1024),
    )
    .expect("creating UDP socket failed");

    let mut tcp_socket = TcpSocket::new(
        StreamSocketArgs::new(nm)
            .send_buffer(64 * 1024)
            .recv_buffer(256 * 1024),
    )
    .expect("creating TCP socket failed");

    udp_socket.bind(1337).expect("bind failed");

    Semaphore::attach("net-udp")
        .expect("attaching to net-udp semaphore failed")
        .up()
        .expect("udp up failed");

    let sem_tcp = Semaphore::attach("net-tcp").expect("attaching to net-tcp semaphore failed");

    let mut buffer = [0u8; 1024];

    let mut waiter = FileWaiter::default();
    waiter.add(tcp_socket.fd(), FileEvent::INPUT);
    waiter.add(udp_socket.fd(), FileEvent::INPUT);

    loop {
        if tcp_socket.state() == State::Closed {
            tcp_socket.listen(1338).expect("listen failed");
            sem_tcp.up().expect("tcp up failed");
        }

        if udp_socket.has_data() {
            // ignore errors
            if let Ok((size, src)) = udp_socket.recv_from(&mut buffer) {
                udp_socket.send_to(&buffer[0..size], src).ok();
            }
        }

        if tcp_socket.has_data() {
            // ignore errors
            if let Ok(size) = tcp_socket.recv(&mut buffer) {
                tcp_socket.send(&buffer[0..size]).ok();
            }
        }

        if !udp_socket.has_data() && !tcp_socket.has_data() {
            if tcp_socket.state() == State::RemoteClosed {
                tcp_socket.abort().unwrap();
            }
            else {
                // we only care about input here, because we operate in blocking mode and only want
                // to know if any of the sockets might return true for has_data. Sending is done in
                // blocking mode so that we simply wait there until that's possible.
                waiter.wait();
            }
        }
    }
}
