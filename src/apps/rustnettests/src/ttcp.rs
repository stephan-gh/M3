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

use m3::boxed::Box;
use m3::com::Semaphore;
use m3::errors::Code;
use m3::net::{IpAddr, State, StreamSocketArgs, TcpSocket};
use m3::pes::{Activity, VPEArgs, PE, VPE};
use m3::session::{NetworkDirection, NetworkManager};
use m3::test;
use m3::{wv_assert_eq, wv_assert_err, wv_assert_ok, wv_run_test};

pub fn run(t: &mut dyn test::WvTester) {
    wv_run_test!(t, basics);
    wv_run_test!(t, unreachable);
    wv_run_test!(t, open_close);
    wv_run_test!(t, receive_after_close);
    wv_run_test!(t, data);
}

fn unreachable() {
    let nm = wv_assert_ok!(NetworkManager::new("net0"));

    let mut socket = wv_assert_ok!(TcpSocket::new(StreamSocketArgs::new(&nm)));

    wv_assert_err!(socket.connect(IpAddr::new(127, 0, 0, 1), 80), Code::ConnectionFailed);
}

fn basics() {
    let nm = wv_assert_ok!(NetworkManager::new("net0"));

    let mut socket = wv_assert_ok!(TcpSocket::new(StreamSocketArgs::new(&nm)));

    wv_assert_eq!(socket.state(), State::Closed);

    wv_assert_ok!(Semaphore::attach("net-tcp").unwrap().down());

    wv_assert_err!(socket.send(&[0]), Code::NotConnected);
    wv_assert_ok!(socket.connect(IpAddr::new(192, 168, 112, 1), 1338));
    wv_assert_eq!(socket.state(), State::Connected);

    let mut buf = [0u8; 32];
    wv_assert_ok!(socket.send(&buf));
    wv_assert_ok!(socket.recv(&mut buf));

    // connecting to the same remote endpoint is okay
    wv_assert_ok!(socket.connect(IpAddr::new(192, 168, 112, 1), 1338));
    // if anything differs, it's an error
    wv_assert_err!(
        socket.connect(IpAddr::new(192, 168, 112, 1), 1339),
        Code::IsConnected
    );
    wv_assert_err!(
        socket.connect(IpAddr::new(192, 168, 112, 2), 1338),
        Code::IsConnected
    );

    wv_assert_ok!(socket.abort());
    wv_assert_eq!(socket.state(), State::Closed);
}

fn open_close() {
    let nm = wv_assert_ok!(NetworkManager::new("net0"));

    let mut socket = wv_assert_ok!(TcpSocket::new(StreamSocketArgs::new(&nm)));

    wv_assert_ok!(Semaphore::attach("net-tcp").unwrap().down());

    wv_assert_ok!(socket.connect(IpAddr::new(192, 168, 112, 1), 1338));
    wv_assert_eq!(socket.state(), State::Connected);

    wv_assert_ok!(socket.close());
    wv_assert_eq!(socket.state(), State::Closed);

    let mut buf = [0u8; 32];
    wv_assert_err!(socket.send(&buf), Code::NotConnected);
    wv_assert_err!(socket.recv(&mut buf), Code::NotConnected);
}

fn receive_after_close() {
    let pe = wv_assert_ok!(PE::new(VPE::cur().pe_desc()));
    let mut vpe = wv_assert_ok!(VPE::new_with(pe, VPEArgs::new("tcp-server")));

    let sem = wv_assert_ok!(Semaphore::create(0));
    let sem_sel = sem.sel();
    wv_assert_ok!(vpe.delegate_obj(sem_sel));

    let act = wv_assert_ok!(vpe.run(Box::new(move || {
        let sem = Semaphore::bind(sem_sel);

        let nm = wv_assert_ok!(NetworkManager::new("net1"));

        let mut socket = wv_assert_ok!(TcpSocket::new(StreamSocketArgs::new(&nm)));

        wv_assert_ok!(socket.listen(3000));
        wv_assert_eq!(socket.state(), State::Listening);
        wv_assert_ok!(sem.up());

        let (ip, _port) = wv_assert_ok!(socket.accept());
        wv_assert_eq!(ip, IpAddr::new(192, 168, 112, 2));
        wv_assert_eq!(socket.state(), State::Connected);

        let mut buf = [0u8; 32];
        wv_assert_eq!(socket.recv(&mut buf), Ok(32));
        wv_assert_ok!(socket.send(&buf));

        wv_assert_ok!(socket.close());
        wv_assert_eq!(socket.state(), State::Closed);

        0
    })));

    let nm = wv_assert_ok!(NetworkManager::new("net0"));

    let mut socket = wv_assert_ok!(TcpSocket::new(StreamSocketArgs::new(&nm)));

    wv_assert_ok!(sem.down());

    wv_assert_ok!(socket.connect(IpAddr::new(192, 168, 112, 1), 3000));

    let mut buf = [0u8; 32];
    wv_assert_ok!(socket.send(&buf));
    wv_assert_eq!(socket.recv(&mut buf), Ok(32));

    // at some point, the socket should receive the closed event from the remote side
    while socket.state() != State::RemoteClosed {
        nm.wait(NetworkDirection::INPUT);
    }

    wv_assert_ok!(socket.close());

    wv_assert_eq!(act.wait(), Ok(0));
}

fn data() {
    let nm = wv_assert_ok!(NetworkManager::new("net0"));

    let mut socket = wv_assert_ok!(TcpSocket::new(StreamSocketArgs::new(&nm)));

    wv_assert_ok!(Semaphore::attach("net-tcp").unwrap().down());

    wv_assert_ok!(socket.connect(IpAddr::new(192, 168, 112, 1), 1338));

    let mut send_buf = [0u8; 1024];
    for i in 0..1024 {
        send_buf[i] = i as u8;
    }

    let mut recv_buf = [0u8; 1024];

    let packet_sizes = [8, 16, 32, 64, 128, 256, 512, 1024];

    for pkt_size in &packet_sizes {
        wv_assert_ok!(socket.send(&send_buf[0..*pkt_size]));

        let mut received = 0;
        let mut expected_byte: u8 = 0;
        while received < *pkt_size {
            let recv_size = wv_assert_ok!(socket.recv(&mut recv_buf));

            for i in 0..recv_size {
                wv_assert_eq!(recv_buf[i], expected_byte);
                expected_byte = expected_byte.wrapping_add(1);
            }
            received += recv_size;
        }
    }
}
