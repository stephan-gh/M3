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

use m3::io::{Read, Write};
use m3::test::WvTester;
use m3::vfs::{FileRef, OpenFlags, VFS};
use m3::{vec, wv_assert_eq, wv_assert_ok, wv_run_test};

pub fn run(t: &mut dyn WvTester) {
    wv_assert_ok!(VFS::mount("/", "m3fs", "m3fs"));

    wv_run_test!(t, text_files);
    wv_run_test!(t, pat_file);
    wv_run_test!(t, write_file);
}

fn text_files() {
    {
        let mut file = wv_assert_ok!(VFS::open("/test.txt", OpenFlags::R));
        let s = wv_assert_ok!(file.read_to_string());
        wv_assert_eq!(s, "This is a test\n");
    }

    {
        let mut file = wv_assert_ok!(VFS::open("/test/test.txt", OpenFlags::R));
        let s = wv_assert_ok!(file.read_to_string());
        wv_assert_eq!(s, "This is a test\n");
    }
}

fn pat_file() {
    let mut file = wv_assert_ok!(VFS::open("/pat.bin", OpenFlags::R));
    let mut buf = vec![0u8; 8 * 1024];

    wv_assert_eq!(_validate_pattern_content(&mut file, &mut buf), 64 * 1024);
}

fn write_file() {
    // create new file
    {
        let mut file = wv_assert_ok!(VFS::open(
            "/newfile",
            OpenFlags::W | OpenFlags::CREATE | OpenFlags::TRUNC
        ));
        wv_assert_ok!(write!(file, "my content is {:#x}", 0x1234));
        // ensure it's written to disk
        wv_assert_ok!(file.sync());
    }

    // read content back
    {
        let mut file = wv_assert_ok!(VFS::open("/newfile", OpenFlags::R));
        let s = wv_assert_ok!(file.read_to_string());
        wv_assert_eq!(s, "my content is 0x1234");
    }
}

fn _validate_pattern_content(file: &mut FileRef, mut buf: &mut [u8]) -> usize {
    let mut pos: usize = 0;
    loop {
        let count = wv_assert_ok!(file.read(&mut buf));
        if count == 0 {
            break;
        }

        for b in buf.iter().take(count) {
            wv_assert_eq!(
                *b,
                (pos & 0xFF) as u8,
                "content wrong at offset {}: {} vs. {}",
                pos,
                *b,
                (pos & 0xFF) as u8
            );
            pos += 1;
        }
    }
    pos
}
