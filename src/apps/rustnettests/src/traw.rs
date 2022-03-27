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
use m3::io;
use m3::kif;
use m3::session::Pipes;
use m3::test;
use m3::tiles::{Activity, ActivityArgs, RunningActivity, Tile};
use m3::vfs::{BufReader, IndirectPipe};
use m3::{format, wv_assert, wv_assert_eq, wv_assert_ok, wv_run_test};

pub fn run(t: &mut dyn test::WvTester) {
    wv_run_test!(t, exec_ping);
}

fn exec_ping() {
    let pipeserv = wv_assert_ok!(Pipes::new("pipes"));
    let pipe_mem = wv_assert_ok!(MemGate::new(0x10000, kif::Perm::RW));
    let pipe = wv_assert_ok!(IndirectPipe::new(&pipeserv, &pipe_mem, 0x10000));

    let tile = wv_assert_ok!(Tile::get("clone|own"));
    let mut ping = wv_assert_ok!(Activity::new_with(tile, ActivityArgs::new("ping")));
    ping.files().set(
        io::STDOUT_FILENO,
        Activity::cur().files().get(pipe.writer_fd()).unwrap(),
    );

    let ping_act = wv_assert_ok!(ping.exec(&["/bin/ping", &crate::NET1_IP.get().to_string()]));

    pipe.close_writer();

    let input = Activity::cur().files().get_ref(pipe.reader_fd()).unwrap();
    let mut reader = BufReader::new(input);
    let mut line = String::new();
    while reader.read_line(&mut line).is_ok() {
        if line.is_empty() {
            break;
        }

        if line.contains("packets") {
            wv_assert!(line.starts_with("5 packets transmitted, 5 received"));
        }
        else if line.contains("from") {
            wv_assert!(line.starts_with(&format!("84 bytes from {}:", crate::NET1_IP.get())));
        }

        m3::println!("{}", line);
        line.clear();
    }

    pipe.close_reader();

    wv_assert_eq!(ping_act.wait(), Ok(0));
}
