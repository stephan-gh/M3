/*
 * Copyright (C) 2021-2022 Nils Asmussen, Barkhausen Institut
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

use m3::cap::Selector;
use m3::com::Semaphore;
use m3::errors::Code;
use m3::net::{Endpoint, IpAddr, Socket, State, StreamSocket, StreamSocketArgs, TcpSocket};
use m3::session::NetworkManager;
use m3::test::{DefaultWvTester, WvTester};
use m3::tiles::{Activity, ActivityArgs, ChildActivity, RunningActivity, Tile};
use m3::vec::Vec;
use m3::vfs::{File, FileEvent, FileWaiter};
use m3::{vec, wv_assert_eq, wv_assert_err, wv_assert_ok, wv_run_test};

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, basics);
    wv_run_test!(t, unreachable);
    wv_run_test!(t, nonblocking_client);
    wv_run_test!(t, nonblocking_server);
    wv_run_test!(t, open_close);
    wv_run_test!(t, receive_after_close);
    wv_run_test!(t, data);
}

fn basics(t: &mut dyn WvTester) {
    let nm = wv_assert_ok!(NetworkManager::new("net0"));

    let mut socket = wv_assert_ok!(TcpSocket::new(StreamSocketArgs::new(nm)));

    wv_assert_eq!(t, socket.state(), State::Closed);
    wv_assert_eq!(t, socket.local_endpoint(), None);
    wv_assert_eq!(t, socket.remote_endpoint(), None);

    wv_assert_ok!(Semaphore::attach("net-tcp").unwrap().down());

    wv_assert_err!(t, socket.send(&[0]), Code::NotConnected);
    wv_assert_ok!(socket.connect(Endpoint::new(crate::DST_IP.get(), 1338)));
    wv_assert_eq!(t, socket.state(), State::Connected);
    wv_assert_eq!(
        t,
        socket.local_endpoint().unwrap().addr,
        crate::NET0_IP.get()
    );
    wv_assert_eq!(
        t,
        socket.remote_endpoint(),
        Some(Endpoint::new(crate::DST_IP.get(), 1338))
    );

    let mut buf = [0u8; 32];
    wv_assert_eq!(t, socket.send(&buf), Ok(buf.len()));
    wv_assert_ok!(socket.recv(&mut buf));

    // connecting to the same remote endpoint is okay
    wv_assert_ok!(socket.connect(Endpoint::new(crate::DST_IP.get(), 1338)));
    // if anything differs, it's an error
    wv_assert_err!(
        t,
        socket.connect(Endpoint::new(crate::DST_IP.get(), 1339)),
        Code::IsConnected
    );
    wv_assert_err!(
        t,
        socket.connect(Endpoint::new(IpAddr::new(88, 87, 86, 85), 1338)),
        Code::IsConnected
    );

    wv_assert_ok!(socket.abort());
    wv_assert_eq!(t, socket.state(), State::Closed);
    wv_assert_eq!(t, socket.local_endpoint(), None);
    wv_assert_eq!(t, socket.remote_endpoint(), None);
}

fn unreachable(t: &mut dyn WvTester) {
    let nm = wv_assert_ok!(NetworkManager::new("net0"));

    let mut socket = wv_assert_ok!(TcpSocket::new(StreamSocketArgs::new(nm)));

    wv_assert_err!(
        t,
        socket.connect(Endpoint::new(IpAddr::new(88, 87, 86, 85), 80)),
        Code::ConnectionFailed
    );
}

fn nonblocking_client(t: &mut dyn WvTester) {
    let nm = wv_assert_ok!(NetworkManager::new("net0"));

    let mut socket = wv_assert_ok!(TcpSocket::new(StreamSocketArgs::new(nm)));

    wv_assert_ok!(Semaphore::attach("net-tcp").unwrap().down());

    wv_assert_ok!(socket.set_blocking(false));

    let mut in_waiter = FileWaiter::default();
    in_waiter.add(socket.fd(), FileEvent::INPUT);
    let mut out_waiter = FileWaiter::default();
    out_waiter.add(socket.fd(), FileEvent::OUTPUT);

    wv_assert_err!(
        t,
        socket.connect(Endpoint::new(crate::DST_IP.get(), 1338)),
        Code::InProgress
    );
    while socket.state() != State::Connected {
        wv_assert_eq!(t, socket.state(), State::Connecting);
        wv_assert_err!(
            t,
            socket.connect(Endpoint::new(crate::DST_IP.get(), 1338)),
            Code::AlreadyInProgress
        );
        in_waiter.wait();
    }

    let mut buf = [0u8; 32];

    for _ in 0..8 {
        while let Err(e) = socket.send(&buf) {
            wv_assert_eq!(t, e.code(), Code::WouldBlock);
            out_waiter.wait();
        }
    }

    let mut total = 0;
    while total < 8 * buf.len() {
        loop {
            match socket.recv(&mut buf) {
                Err(e) => wv_assert_eq!(t, e.code(), Code::WouldBlock),
                Ok(size) => {
                    total += size;
                    break;
                },
            }
            in_waiter.wait();
        }
    }
    wv_assert_eq!(t, total, 8 * buf.len());

    while let Err(e) = socket.close() {
        if e.code() != Code::WouldBlock {
            wv_assert_eq!(t, e.code(), Code::InProgress);
            break;
        }
        out_waiter.wait();
    }

    while socket.state() != State::Closed {
        wv_assert_eq!(t, socket.state(), State::Closing);
        wv_assert_err!(t, socket.close(), Code::AlreadyInProgress);
        in_waiter.wait();
    }
}

fn nonblocking_server(t: &mut dyn WvTester) {
    let tile = wv_assert_ok!(Tile::get("compat|own"));
    let mut act = wv_assert_ok!(ChildActivity::new_with(
        tile,
        ActivityArgs::new("tcp-server")
    ));

    let sem = wv_assert_ok!(Semaphore::create(0));
    wv_assert_ok!(act.delegate_obj(sem.sel()));

    let mut dst = act.data_sink();
    dst.push(sem.sel());
    dst.push(&m3::format!("{}", crate::NET0_IP.get()));
    dst.push(&m3::format!("{}", crate::NET1_IP.get()));

    let act = wv_assert_ok!(act.run(|| {
        let mut t = DefaultWvTester::default();
        let mut src = Activity::own().data_source();
        let sem_sel: Selector = src.pop().unwrap();
        let net0_ip: IpAddr = src.pop::<&str>().unwrap().parse().unwrap();
        let net1_ip: IpAddr = src.pop::<&str>().unwrap().parse().unwrap();

        let sem = Semaphore::bind(sem_sel);

        let nm = wv_assert_ok!(NetworkManager::new("net1"));

        let mut socket = wv_assert_ok!(TcpSocket::new(StreamSocketArgs::new(nm)));
        wv_assert_ok!(socket.set_blocking(false));

        wv_assert_eq!(t, socket.local_endpoint(), None);
        wv_assert_eq!(t, socket.remote_endpoint(), None);

        wv_assert_ok!(socket.listen(3000));
        wv_assert_eq!(t, socket.state(), State::Listening);
        wv_assert_ok!(sem.up());

        let mut waiter = FileWaiter::default();
        waiter.add(socket.fd(), FileEvent::INPUT);

        wv_assert_err!(t, socket.accept(), Code::InProgress);
        while socket.state() == State::Connecting {
            wv_assert_err!(t, socket.accept(), Code::AlreadyInProgress);
            waiter.wait();
        }
        assert!(socket.state() == State::Connected || socket.state() == State::RemoteClosed);

        wv_assert_eq!(
            t,
            socket.local_endpoint(),
            Some(Endpoint::new(net1_ip, 3000))
        );
        wv_assert_eq!(t, socket.remote_endpoint().unwrap().addr, net0_ip);

        wv_assert_ok!(socket.set_blocking(true));
        wv_assert_ok!(socket.close());

        Ok(())
    }));

    let nm = wv_assert_ok!(NetworkManager::new("net0"));

    let mut socket = wv_assert_ok!(TcpSocket::new(StreamSocketArgs::new(nm)));

    wv_assert_ok!(sem.down());

    wv_assert_ok!(socket.connect(Endpoint::new(crate::NET1_IP.get(), 3000)));

    wv_assert_ok!(socket.close());

    wv_assert_eq!(t, act.wait(), Ok(Code::Success));
}

fn open_close(t: &mut dyn WvTester) {
    let nm = wv_assert_ok!(NetworkManager::new("net0"));

    let mut socket = wv_assert_ok!(TcpSocket::new(StreamSocketArgs::new(nm)));

    wv_assert_ok!(Semaphore::attach("net-tcp").unwrap().down());

    wv_assert_ok!(socket.connect(Endpoint::new(crate::DST_IP.get(), 1338)));
    wv_assert_eq!(t, socket.state(), State::Connected);

    wv_assert_ok!(socket.close());
    wv_assert_eq!(t, socket.state(), State::Closed);
    wv_assert_eq!(t, socket.local_endpoint(), None);
    wv_assert_eq!(t, socket.remote_endpoint(), None);

    let mut buf = [0u8; 32];
    wv_assert_err!(t, socket.send(&buf), Code::NotConnected);
    wv_assert_err!(t, socket.recv(&mut buf), Code::NotConnected);
}

fn receive_after_close(t: &mut dyn WvTester) {
    let tile = wv_assert_ok!(Tile::get("compat|own"));
    let mut act = wv_assert_ok!(ChildActivity::new_with(
        tile,
        ActivityArgs::new("tcp-server")
    ));

    let sem = wv_assert_ok!(Semaphore::create(0));
    wv_assert_ok!(act.delegate_obj(sem.sel()));

    let mut dst = act.data_sink();
    dst.push(sem.sel());
    dst.push(&m3::format!("{}", crate::NET0_IP.get()));

    let act = wv_assert_ok!(act.run(|| {
        let mut t = DefaultWvTester::default();
        let mut src = Activity::own().data_source();
        let sem_sel: Selector = src.pop().unwrap();
        let net0_ip: IpAddr = src.pop::<&str>().unwrap().parse().unwrap();

        let sem = Semaphore::bind(sem_sel);

        let nm = wv_assert_ok!(NetworkManager::new("net1"));

        let mut socket = wv_assert_ok!(TcpSocket::new(StreamSocketArgs::new(nm)));

        wv_assert_ok!(socket.listen(3000));
        wv_assert_eq!(t, socket.state(), State::Listening);
        wv_assert_ok!(sem.up());

        let ep = wv_assert_ok!(socket.accept());
        wv_assert_eq!(t, ep.addr, net0_ip);
        wv_assert_eq!(t, socket.state(), State::Connected);

        let mut buf = [0u8; 32];
        wv_assert_eq!(t, socket.recv(&mut buf), Ok(32));
        wv_assert_eq!(t, socket.send(&buf), Ok(32));

        wv_assert_ok!(socket.close());
        wv_assert_eq!(t, socket.state(), State::Closed);

        Ok(())
    }));

    let nm = wv_assert_ok!(NetworkManager::new("net0"));

    let mut socket = wv_assert_ok!(TcpSocket::new(StreamSocketArgs::new(nm)));

    wv_assert_ok!(sem.down());

    wv_assert_ok!(socket.connect(Endpoint::new(crate::DST_IP.get(), 3000)));

    let mut buf = [0u8; 32];
    wv_assert_eq!(t, socket.send(&buf), Ok(32));
    wv_assert_eq!(t, socket.recv(&mut buf), Ok(32));

    let mut waiter = FileWaiter::default();
    waiter.add(socket.fd(), FileEvent::INPUT);

    // at some point, the socket should receive the closed event from the remote side
    while socket.state() != State::RemoteClosed {
        waiter.wait();
    }

    wv_assert_ok!(socket.close());

    wv_assert_eq!(t, act.wait(), Ok(Code::Success));
}

fn data(t: &mut dyn WvTester) {
    let nm = wv_assert_ok!(NetworkManager::new("net0"));

    let mut socket = wv_assert_ok!(TcpSocket::new(
        StreamSocketArgs::new(nm).send_buffer(2 * 1024)
    ));

    wv_assert_ok!(Semaphore::attach("net-tcp").unwrap().down());

    wv_assert_ok!(socket.connect(Endpoint::new(crate::DST_IP.get(), 1338)));

    // disable 256 to workaround the bug in gem5's E1000 model
    let packet_sizes = [8, 16, 32, 64, 128, /*256,*/ 512, 934, 1024, 2048, 4096];

    for pkt_size in &packet_sizes {
        let mut send_buf = Vec::with_capacity(pkt_size * 8);
        for i in 0..pkt_size * 8 {
            send_buf.push(i as u8);
        }
        let mut recv_buf = vec![0u8; *pkt_size];

        for i in 0..8 {
            let pkt = &send_buf[pkt_size * i..pkt_size * (i + 1)];
            wv_assert_eq!(t, socket.send(pkt), Ok(pkt.len()));
        }

        let mut received = 0;
        let mut expected_byte: u8 = 0;
        while received < *pkt_size * 8 {
            let recv_size = wv_assert_ok!(socket.recv(&mut recv_buf));

            for bufi in recv_buf.iter().take(recv_size) {
                wv_assert_eq!(t, *bufi, expected_byte);
                expected_byte = expected_byte.wrapping_add(1);
            }
            received += recv_size;
        }
    }
}
