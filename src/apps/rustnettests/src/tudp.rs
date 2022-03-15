/*
 * Copyright (C) 2021 Nils Asmussen, Barkhausen Institut
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
use m3::net::{DgramSocketArgs, Endpoint, State, UdpSocket};
use m3::session::NetworkManager;
use m3::test;
use m3::{wv_assert_eq, wv_assert_err, wv_assert_ok, wv_run_test};

pub fn run(t: &mut dyn test::WvTester) {
    // wait once for UDP, because it's connection-less
    wv_assert_ok!(Semaphore::attach("net-udp").unwrap().down());

    wv_run_test!(t, basics);
    wv_run_test!(t, connect);
    wv_run_test!(t, data);
}

fn basics() {
    let nm = wv_assert_ok!(NetworkManager::new("net0"));

    let mut socket = wv_assert_ok!(UdpSocket::new(DgramSocketArgs::new(&nm)));

    wv_assert_eq!(socket.state(), State::Closed);
    wv_assert_eq!(socket.local_endpoint(), None);

    wv_assert_ok!(socket.bind(2000));
    wv_assert_eq!(socket.state(), State::Bound);
    wv_assert_eq!(
        socket.local_endpoint(),
        Some(Endpoint::new(crate::NET0_IP.get(), 2000))
    );

    wv_assert_err!(socket.bind(2001), Code::InvState);
}

fn connect() {
    let nm = wv_assert_ok!(NetworkManager::new("net0"));

    let mut socket = wv_assert_ok!(UdpSocket::new(DgramSocketArgs::new(&nm)));

    wv_assert_eq!(socket.state(), State::Closed);
    wv_assert_eq!(socket.local_endpoint(), None);

    wv_assert_ok!(socket.connect(Endpoint::new(crate::NET0_IP.get(), 2000)));
    wv_assert_eq!(socket.state(), State::Bound);
}

fn data() {
    let nm = wv_assert_ok!(NetworkManager::new("net0"));

    let mut socket = wv_assert_ok!(UdpSocket::new(DgramSocketArgs::new(&nm)));

    let dest = Endpoint::new(crate::DST_IP.get(), 1337);

    let mut send_buf = [0u8; 1024];
    for (i, bufi) in send_buf.iter_mut().enumerate() {
        *bufi = i as u8;
    }

    let mut recv_buf = [0u8; 1024];

    let packet_sizes = [8, 16, 32, 64, 128, 256, 512, 1024];

    for pkt_size in &packet_sizes {
        wv_assert_ok!(socket.send_to(&send_buf[0..*pkt_size], dest));

        let (recv_size, src) = wv_assert_ok!(socket.recv_from(&mut recv_buf));

        wv_assert_eq!(*pkt_size, recv_size as usize);
        wv_assert_eq!(src, dest);

        wv_assert_eq!(&recv_buf[0..recv_size], &send_buf[0..recv_size]);
    }
}
