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

use m3::client::Pipes;
use m3::com::MemGate;
use m3::errors::Code;
use m3::io::{Read, Write};
use m3::kif;
use m3::mem::GlobOff;
use m3::test::WvTester;
use m3::tiles::OwnActivity;
use m3::vfs::{File, IndirectPipe, OpenFlags, VFS};
use m3::{wv_assert_eq, wv_assert_err, wv_assert_ok, wv_assert_some, wv_run_test};

const PIPE_SIZE: usize = 16;
const DATA_SIZE: usize = PIPE_SIZE / 4;

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, files);
    wv_run_test!(t, pipes);
}

fn files(t: &mut dyn WvTester) {
    let mut fin = wv_assert_ok!(VFS::open("/mat.txt", OpenFlags::R));
    let mut fout = wv_assert_ok!(VFS::open(
        "/nonblocking-res.txt",
        OpenFlags::CREATE | OpenFlags::W
    ));

    wv_assert_err!(t, fin.set_blocking(false), Code::NotSup);
    wv_assert_err!(t, fout.set_blocking(false), Code::NotSup);

    let send_data: [u8; DATA_SIZE] = *b"test";
    let mut recv_data = [0u8; DATA_SIZE];

    loop {
        let mut progress = 0;

        if let Ok(read) = fin.read(&mut recv_data) {
            if read == 0 {
                break;
            }
            progress += 1;
        }

        if fout.write(&send_data).is_ok() {
            progress += 1;
        }

        if progress == 0 {
            OwnActivity::sleep().ok();
        }
    }
}

fn pipes(t: &mut dyn WvTester) {
    let pipeserv = wv_assert_ok!(Pipes::new("pipes"));
    let pipe_mem = wv_assert_ok!(MemGate::new(PIPE_SIZE as GlobOff, kif::Perm::RW));
    let pipe = wv_assert_ok!(IndirectPipe::new(&pipeserv, pipe_mem));

    let mut fin = wv_assert_some!(pipe.reader());
    let mut fout = wv_assert_some!(pipe.writer());
    wv_assert_ok!(fin.set_blocking(false));
    wv_assert_ok!(fout.set_blocking(false));

    let send_data: [u8; DATA_SIZE] = *b"test";
    let mut recv_data = [0u8; DATA_SIZE];

    let mut count = 0;
    while count < 100 {
        let mut progress = 0;

        if let Ok(read) = fin.read(&mut recv_data) {
            // this is actually not guaranteed, but depends on the implementation of the pipe
            // server. however, we want to ensure that the read data is correct, which is difficult
            // otherwise.
            wv_assert_eq!(t, read, send_data.len());
            wv_assert_eq!(t, recv_data, send_data);
            progress += 1;
            count += read;
        }

        if let Ok(written) = fout.write(&send_data) {
            // see above
            wv_assert_eq!(t, written, send_data.len());
            progress += 1;
        }

        if count < 100 && progress == 0 {
            OwnActivity::sleep().ok();
        }
    }
}
