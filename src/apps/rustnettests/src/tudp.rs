/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

use m3::com::Semaphore;
use m3::errors::Code;
use m3::net::{IpAddr, State, UdpSocket};
use m3::session::NetworkManager;
use m3::test;
use m3::{wv_assert_eq, wv_assert_err, wv_assert_ok, wv_run_test};

pub fn run(t: &mut dyn test::WvTester) {
    // wait once for UDP, because it's connection-less
    wv_assert_ok!(Semaphore::attach("net-udp").unwrap().down());

    wv_run_test!(t, basics);
    wv_run_test!(t, data);
}

fn basics() {
    let nm = wv_assert_ok!(NetworkManager::new("net0"));

    let mut socket = wv_assert_ok!(UdpSocket::new(&nm));

    wv_assert_eq!(socket.state(), State::Closed);

    wv_assert_ok!(socket.bind(IpAddr::new(192, 168, 112, 2), 1337));
    wv_assert_eq!(socket.state(), State::Bound);

    wv_assert_err!(
        socket.bind(IpAddr::new(192, 168, 112, 2), 1338),
        Code::InvState
    );

    wv_assert_ok!(socket.abort());
    wv_assert_eq!(socket.state(), State::Closed);
}

fn data() {
    let nm = wv_assert_ok!(NetworkManager::new("net0"));

    let mut socket = wv_assert_ok!(UdpSocket::new(&nm));
    wv_assert_ok!(socket.bind(IpAddr::new(192, 168, 112, 2), 1338));

    let dest_addr = IpAddr::new(192, 168, 112, 1);
    let dest_port = 1337;

    let mut send_buf = [0u8; 1024];
    for i in 0..1024 {
        send_buf[i] = i as u8;
    }

    let mut recv_buf = [0u8; 1024];

    let packet_sizes = [8, 16, 32, 64, 128, 256, 512, 1024];

    for pkt_size in &packet_sizes {
        wv_assert_ok!(socket.send_to(&send_buf[0..*pkt_size], dest_addr, dest_port));

        let (recv_size, ip, port) = wv_assert_ok!(socket.recv_from(&mut recv_buf));

        wv_assert_eq!(*pkt_size, recv_size as usize);
        wv_assert_eq!(ip, dest_addr);
        wv_assert_eq!(port, dest_port);

        wv_assert_eq!(&recv_buf[0..recv_size], &send_buf[0..recv_size]);
    }
}
