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

use m3::col::{String, ToString};
use m3::com::MemGate;
use m3::errors::Code;
use m3::io;
use m3::kif;
use m3::net::{RawSocket, RawSocketArgs, MAC};
use m3::session::{NetworkManager, Pipes};
use m3::test::WvTester;
use m3::tiles::{ActivityArgs, ChildActivity, RunningActivity, Tile};
use m3::vfs::{BufReader, IndirectPipe};
use m3::{format, wv_assert, wv_assert_eq, wv_assert_err, wv_assert_ok, wv_run_test};

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, no_perm);
    wv_run_test!(t, mac_addr);
    wv_run_test!(t, exec_ping);
}

fn no_perm(t: &mut dyn WvTester) {
    let nm = wv_assert_ok!(NetworkManager::new("net0"));

    wv_assert_err!(
        t,
        RawSocket::new(RawSocketArgs::new(nm), None).map(|_| ()),
        Code::NoPerm
    );
}

fn mac_addr(t: &mut dyn WvTester) {
    let mac = MAC::new(0x01, 0x02, 0x03, 0x04, 0x05, 0x06);
    wv_assert_eq!(t, mac.raw(), 0x060504030201);
    wv_assert_eq!(t, format!("{}", mac), "01:02:03:04:05:06");
}

fn exec_ping(t: &mut dyn WvTester) {
    let pipeserv = wv_assert_ok!(Pipes::new("pipes"));
    let pipe_mem = wv_assert_ok!(MemGate::new(0x10000, kif::Perm::RW));
    let pipe = wv_assert_ok!(IndirectPipe::new(&pipeserv, &pipe_mem, 0x10000));

    let tile = wv_assert_ok!(Tile::get("compat|own"));
    let mut ping = wv_assert_ok!(ChildActivity::new_with(tile, ActivityArgs::new("ping")));
    ping.add_file(io::STDOUT_FILENO, pipe.writer().unwrap().fd());

    let ping_act = wv_assert_ok!(ping.exec(&["/bin/ping", &crate::NET1_IP.get().to_string()]));

    pipe.close_writer();

    let input = pipe.reader().unwrap();
    let mut reader = BufReader::new(input);
    let mut line = String::new();
    while reader.read_line(&mut line).is_ok() {
        if line.is_empty() {
            break;
        }

        if line.contains("packets") {
            wv_assert!(t, line.starts_with("5 packets transmitted, 5 received"));
        }
        else if line.contains("from") {
            wv_assert!(
                t,
                line.starts_with(&format!("84 bytes from {}:", crate::NET1_IP.get()))
            );
        }

        m3::println!("{}", line);
        line.clear();
    }

    pipe.close_reader();

    wv_assert_eq!(t, ping_act.wait(), Ok(Code::Success));
}
