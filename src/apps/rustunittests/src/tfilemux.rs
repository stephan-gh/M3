/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

use core::cmp;
use m3::client::Pipes;
use m3::com::MemGate;
use m3::io::{Read, Write};
use m3::kif;
use m3::mem::GlobOff;
use m3::test::WvTester;
use m3::vfs::{BufReader, FileRef, GenericFile, IndirectPipe, OpenFlags, VFS};
use m3::{vec, wv_assert_eq, wv_assert_ok, wv_run_test};

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, genfile_mux);
    wv_run_test!(t, pipe_mux);
}

fn genfile_mux(t: &mut dyn WvTester) {
    const NUM: usize = 2;
    const STEP_SIZE: usize = 400;
    const FILE_SIZE: usize = 12 * 1024;

    let mut files = vec![];
    for _ in 0..NUM {
        let file = wv_assert_ok!(VFS::open("/pat.bin", OpenFlags::R));
        files.push(BufReader::new(file));
    }

    let mut pos = 0;
    while pos < FILE_SIZE {
        for f in &mut files {
            let end = cmp::min(FILE_SIZE, pos + STEP_SIZE);
            for tpos in pos..end {
                let mut buf = [0u8];
                wv_assert_eq!(t, f.read(&mut buf), Ok(1));
                wv_assert_eq!(t, buf[0], (tpos & 0xFF) as u8);
            }
        }

        pos += STEP_SIZE;
    }
}

fn pipe_mux(t: &mut dyn WvTester) {
    const NUM: usize = 2;
    const STEP_SIZE: usize = 16;
    const DATA_SIZE: usize = 1024;
    const PIPE_SIZE: usize = 256;

    struct Pipe {
        _pipe: IndirectPipe,
        reader: FileRef<GenericFile>,
        writer: FileRef<GenericFile>,
    }

    let pipeserv = wv_assert_ok!(Pipes::new("pipes"));
    let mut pipes = vec![];
    for _ in 0..NUM {
        let mgate = wv_assert_ok!(MemGate::new(PIPE_SIZE as GlobOff, kif::Perm::RW));
        let pipe = wv_assert_ok!(IndirectPipe::new(&pipeserv, mgate));
        pipes.push(Pipe {
            reader: pipe.reader().unwrap(),
            writer: pipe.writer().unwrap(),
            _pipe: pipe,
        });
    }

    let mut src_buf = [0u8; STEP_SIZE];
    for (i, b) in src_buf.iter_mut().enumerate() {
        *b = i as u8;
    }

    let mut pos = 0;
    while pos < DATA_SIZE {
        for p in &mut pipes {
            wv_assert_ok!(p.writer.write(&src_buf));
            wv_assert_ok!(p.writer.flush());
        }

        for p in &mut pipes {
            let mut dst_buf = [0u8; STEP_SIZE];

            wv_assert_ok!(p.reader.read(&mut dst_buf));
            wv_assert_eq!(t, dst_buf, src_buf);
        }

        pos += STEP_SIZE;
    }
}
