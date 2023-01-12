/*
 * Copyright (C) 2022 Nils Asmussen, Barkhausen Institut
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
use m3::errors::{Code, Error};
use m3::test::DefaultWvTester;
use m3::test::WvTester;
use m3::tiles::{ActivityArgs, ChildActivity, OwnActivity, RunningActivity, Tile};
use m3::time::TimeDuration;
use m3::{wv_assert, wv_assert_eq, wv_assert_ok, wv_run_test};

use std::io::{Read, Write};
use std::net::TcpListener;
use std::net::{IpAddr, Ipv4Addr, TcpStream, UdpSocket};

pub fn run(t: &mut dyn WvTester) {
    // wait for UDP socket just once
    Semaphore::attach("net-udp").unwrap().down().unwrap();

    wv_run_test!(t, udp_echo);
    wv_run_test!(t, tcp_echo);
    wv_run_test!(t, tcp_accept);
}

fn udp_echo(t: &mut dyn WvTester) {
    let socket = wv_assert_ok!(UdpSocket::bind("127.0.0.1:3000"));

    let local = wv_assert_ok!(socket.local_addr());
    wv_assert_eq!(t, local.ip(), IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
    wv_assert_eq!(t, local.port(), 3000);

    wv_assert_ok!(socket.connect("127.0.0.1:1337"));

    let peer = wv_assert_ok!(socket.peer_addr());
    wv_assert_eq!(t, peer.ip(), IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
    wv_assert_eq!(t, peer.port(), 1337);

    {
        wv_assert!(t, matches!(socket.send(b"test"), Ok(4)));
        let mut buf = [0u8; 4];
        wv_assert!(t, matches!(socket.recv(&mut buf), Ok(4)));
        wv_assert_eq!(t, buf.as_ref(), b"test");
    }
    {
        wv_assert!(
            t,
            matches!(socket.send_to(b"foobar", "127.0.0.1:1337"), Ok(6))
        );
        let mut buf = [0u8; 6];
        let (res, src) = wv_assert_ok!(socket.recv_from(&mut buf));
        wv_assert_eq!(t, res, 6);
        wv_assert_eq!(t, src.ip(), IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
        wv_assert_eq!(t, src.port(), 1337);
        wv_assert_eq!(t, buf.as_ref(), b"foobar");
    }
}

fn tcp_echo(t: &mut dyn WvTester) {
    Semaphore::attach("net-tcp").unwrap().down().unwrap();

    let mut socket = wv_assert_ok!(TcpStream::connect("127.0.0.1:1338"));

    let local = wv_assert_ok!(socket.local_addr());
    wv_assert_eq!(t, local.ip(), IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));

    let peer = wv_assert_ok!(socket.peer_addr());
    wv_assert_eq!(t, peer.ip(), IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
    wv_assert_eq!(t, peer.port(), 1338);

    {
        wv_assert!(t, matches!(socket.write(b"test"), Ok(4)));
        let mut buf = [0u8; 4];
        wv_assert!(t, matches!(socket.read(&mut buf), Ok(4)));
        wv_assert_eq!(t, buf.as_ref(), b"test");
    }
    {
        wv_assert!(t, matches!(socket.write(b"foobar"), Ok(6)));
        let mut buf = [0u8; 6];
        wv_assert!(t, matches!(socket.read(&mut buf), Ok(6)));
        wv_assert_eq!(t, buf.as_ref(), b"foobar");
    }
}

extern "C" {
    fn __m3c_init_netmng(name: *const i8) -> Code;
}

fn tcp_server() -> Result<(), Error> {
    let mut t = DefaultWvTester::default();
    // connect to netmng explicitly here to specify a different session name
    wv_assert_eq!(
        t,
        unsafe { __m3c_init_netmng(b"netserv\0".as_ptr() as *const i8) },
        Code::Success
    );

    let listener = wv_assert_ok!(TcpListener::bind("127.0.0.1:2000"));

    let (mut stream, peer) = wv_assert_ok!(listener.accept());
    wv_assert_eq!(t, peer.ip(), IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));

    let mut buf = [0u8; 4];
    wv_assert!(t, matches!(stream.read(&mut buf), Ok(4)));
    wv_assert!(t, matches!(stream.write(&buf), Ok(4)));

    Ok(())
}

fn tcp_accept(t: &mut dyn WvTester) {
    Semaphore::attach("net-tcp").unwrap().down().unwrap();

    let tile = wv_assert_ok!(Tile::get("compat|own"));
    let act = wv_assert_ok!(ChildActivity::new_with(tile, ActivityArgs::new("server")));
    let act = wv_assert_ok!(act.run(tcp_server));

    OwnActivity::sleep_for(TimeDuration::from_millis(10)).unwrap();

    // close the socket before we wait for the child
    {
        let mut socket = wv_assert_ok!(TcpStream::connect("127.0.0.1:2000"));

        {
            wv_assert!(t, matches!(socket.write(b"test"), Ok(4)));
            let mut buf = [0u8; 4];
            wv_assert!(t, matches!(socket.read(&mut buf), Ok(4)));
            wv_assert_eq!(t, buf.as_ref(), b"test");
        }
    }

    wv_assert_eq!(t, act.wait(), Ok(Code::Success));
}
