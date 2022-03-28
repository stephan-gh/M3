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

use m3::com::MemGate;
use m3::kif;
use m3::session::Pipes;
use m3::test;
use m3::tiles::Activity;
use m3::vfs::IndirectPipe;
use m3::{wv_assert_eq, wv_assert_ok, wv_assert_some, wv_run_test};

const PIPE_SIZE: usize = 16;
const DATA_SIZE: usize = PIPE_SIZE / 4;

pub fn run(t: &mut dyn test::WvTester) {
    wv_run_test!(t, pipes);
}

fn pipes() {
    let pipeserv = wv_assert_ok!(Pipes::new("pipes"));
    let pipe_mem = wv_assert_ok!(MemGate::new(PIPE_SIZE, kif::Perm::RW));
    let pipe = wv_assert_ok!(IndirectPipe::new(&pipeserv, &pipe_mem, PIPE_SIZE));

    let fin = wv_assert_some!(Activity::cur().files().get(pipe.reader_fd()));
    let fout = wv_assert_some!(Activity::cur().files().get(pipe.writer_fd()));
    wv_assert_ok!(fin.borrow_mut().set_blocking(false));
    wv_assert_ok!(fout.borrow_mut().set_blocking(false));

    let send_data: [u8; DATA_SIZE] = *b"test";
    let mut recv_data = [0u8; DATA_SIZE];

    let mut count = 0;
    while count < 100 {
        let mut progress = 0;

        if let Ok(read) = fin.borrow_mut().read(&mut recv_data) {
            // this is actually not guaranteed, but depends on the implementation of the pipe
            // server. however, we want to ensure that the read data is correct, which is difficult
            // otherwise.
            wv_assert_eq!(read, send_data.len());
            wv_assert_eq!(recv_data, send_data);
            progress += 1;
            count += read;
        }

        if let Ok(written) = fout.borrow_mut().write(&send_data) {
            // see above
            wv_assert_eq!(written, send_data.len());
            progress += 1;
        }

        if count < 100 && progress == 0 {
            Activity::sleep().ok();
        }
    }
}