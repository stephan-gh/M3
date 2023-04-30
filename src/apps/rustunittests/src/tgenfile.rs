/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2021 Nils Asmussen, Barkhausen Institut
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

use m3::col::Vec;
use m3::errors::Code;
use m3::io::{Read, Write};
use m3::test::WvTester;
use m3::vfs::{File, FileRef, GenericFile, OpenFlags, Seek, SeekMode, VFS};
use m3::{vec, wv_assert_eq, wv_assert_err, wv_assert_ok, wv_run_test};

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, permissions);
    wv_run_test!(t, read_string);
    wv_run_test!(t, read_exact);
    wv_run_test!(t, read_file_at_once);
    wv_run_test!(t, read_file_in_small_steps);
    wv_run_test!(t, read_file_in_large_steps);
    wv_run_test!(t, write_and_read_file);
    wv_run_test!(t, write_then_read_file);
    wv_run_test!(t, write_fmt);
    wv_run_test!(t, extend_small_file);
    wv_run_test!(t, overwrite_beginning);
    wv_run_test!(t, truncate);
    wv_run_test!(t, append);
    wv_run_test!(t, append_read);
}

fn permissions(t: &mut dyn WvTester) {
    let filename = "/subdir/subsubdir/testfile.txt";
    let mut buf = [0u8; 16];

    {
        let mut file = wv_assert_ok!(VFS::open(filename, OpenFlags::R));
        wv_assert_err!(t, file.write(&buf), Code::NoPerm);
    }

    {
        let mut file = wv_assert_ok!(VFS::open(filename, OpenFlags::W));
        wv_assert_err!(t, file.read(&mut buf), Code::NoPerm);
    }
}

fn read_string(t: &mut dyn WvTester) {
    let filename = "/subdir/subsubdir/testfile.txt";
    let content = "This is a test!\n";

    let mut file = wv_assert_ok!(VFS::open(filename, OpenFlags::R));

    for i in 0..content.len() {
        wv_assert_eq!(t, file.seek(0, SeekMode::Set), Ok(0));
        let s = wv_assert_ok!(file.read_string(i));
        wv_assert_eq!(t, &s, &content[0..i]);
    }
}

fn read_exact(t: &mut dyn WvTester) {
    let filename = "/subdir/subsubdir/testfile.txt";
    let content = b"This is a test!\n";

    let mut file = wv_assert_ok!(VFS::open(filename, OpenFlags::R));

    let mut buf = [0u8; 32];
    wv_assert_ok!(file.read_exact(&mut buf[0..8]));
    wv_assert_eq!(t, &buf[0..8], &content[0..8]);

    wv_assert_eq!(t, file.seek(0, SeekMode::Set), Ok(0));
    wv_assert_ok!(file.read_exact(&mut buf[0..16]));
    wv_assert_eq!(t, &buf[0..16], &content[0..16]);

    wv_assert_eq!(t, file.seek(0, SeekMode::Set), Ok(0));
    wv_assert_err!(t, file.read_exact(&mut buf), Code::EndOfFile);
}

fn read_file_at_once(t: &mut dyn WvTester) {
    let filename = "/subdir/subsubdir/testfile.txt";

    let mut file = wv_assert_ok!(VFS::open(filename, OpenFlags::R));
    let s = wv_assert_ok!(file.read_to_string());
    wv_assert_eq!(t, s, "This is a test!\n");
}

fn read_file_in_small_steps(t: &mut dyn WvTester) {
    let filename = "/pat.bin";

    let mut file = wv_assert_ok!(VFS::open(filename, OpenFlags::R));
    let mut buf = [0u8; 64];

    wv_assert_eq!(
        t,
        _validate_pattern_content(t, &mut file, &mut buf),
        64 * 1024
    );
}

fn read_file_in_large_steps(t: &mut dyn WvTester) {
    let filename = "/pat.bin";

    let mut file = wv_assert_ok!(VFS::open(filename, OpenFlags::R));
    let mut buf = vec![0u8; 8 * 1024];

    wv_assert_eq!(
        t,
        _validate_pattern_content(t, &mut file, &mut buf),
        64 * 1024
    );
}

fn write_and_read_file(t: &mut dyn WvTester) {
    let content = "Foobar, a test and more and more and more!";
    let filename = "/mat.txt";

    let mut file = wv_assert_ok!(VFS::open(filename, OpenFlags::RW));

    wv_assert_ok!(write!(file, "{}", content));

    wv_assert_eq!(t, file.seek(0, SeekMode::Cur), Ok(content.len()));
    wv_assert_eq!(t, file.seek(0, SeekMode::Set), Ok(0));

    let res = wv_assert_ok!(file.read_string(content.len()));
    wv_assert_eq!(t, &content, &res);

    // undo the write
    let mut old = vec![0u8; content.len()];
    wv_assert_eq!(t, file.seek(0, SeekMode::Set), Ok(0));
    for (i, b) in old.iter_mut().enumerate() {
        *b = i as u8;
    }
    wv_assert_eq!(t, file.write(&old), Ok(content.len()));
}

fn write_then_read_file(t: &mut dyn WvTester) {
    {
        let mut file = wv_assert_ok!(VFS::open("/newfile", OpenFlags::CREATE | OpenFlags::W));
        wv_assert_ok!(write!(file, "Hallo World!"));
    }

    {
        let mut file = wv_assert_ok!(VFS::open("/newfile", OpenFlags::RW));

        // Replace some text
        wv_assert_ok!(write!(file, "Hello "));

        // Read the rest of the text
        let text = wv_assert_ok!(file.read_to_string());
        wv_assert_eq!(t, text, "World!");
    }

    {
        // Check end result
        let mut file = wv_assert_ok!(VFS::open("/newfile", OpenFlags::R));
        let text = wv_assert_ok!(file.read_to_string());
        wv_assert_eq!(t, text, "Hello World!");
    }
}

fn write_fmt(t: &mut dyn WvTester) {
    let mut file = wv_assert_ok!(VFS::open("/newfile", OpenFlags::CREATE | OpenFlags::RW));

    wv_assert_ok!(write!(
        file,
        "This {:.3} is the {}th test of {:#0X}!\n",
        "foobar", 42, 0xAB_CDEF
    ));
    wv_assert_ok!(write!(file, "More formatting: {:?}", Some(Some(1))));

    wv_assert_eq!(t, file.seek(0, SeekMode::Set), Ok(0));

    let s = wv_assert_ok!(file.read_to_string());
    wv_assert_eq!(
        t,
        s,
        "This foo is the 42th test of 0xABCDEF!\nMore formatting: Some(Some(1))"
    );
}

fn extend_small_file(t: &mut dyn WvTester) {
    {
        let mut file = wv_assert_ok!(VFS::open("/test.txt", OpenFlags::W));

        let buf = _get_pat_vector(1024);
        for _ in 0..33 {
            wv_assert_eq!(t, file.write_all(&buf[0..1024]), Ok(()));
        }
    }

    _validate_pattern_file(t, "/test.txt", 1024 * 33);
}

fn overwrite_beginning(t: &mut dyn WvTester) {
    {
        let mut file = wv_assert_ok!(VFS::open("/test.txt", OpenFlags::W));

        let buf = _get_pat_vector(1024);
        for _ in 0..3 {
            wv_assert_eq!(t, file.write_all(&buf[0..1024]), Ok(()));
        }
    }

    _validate_pattern_file(t, "/test.txt", 1024 * 33);
}

fn truncate(t: &mut dyn WvTester) {
    {
        let mut file = wv_assert_ok!(VFS::open("/test.txt", OpenFlags::W | OpenFlags::TRUNC));

        let buf = _get_pat_vector(1024);
        for _ in 0..2 {
            wv_assert_eq!(t, file.write_all(&buf[0..1024]), Ok(()));
        }
    }

    _validate_pattern_file(t, "/test.txt", 1024 * 2);
}

fn append(t: &mut dyn WvTester) {
    {
        let mut file = wv_assert_ok!(VFS::open("/test.txt", OpenFlags::W | OpenFlags::APPEND));
        // TODO perform the seek to end here, because we cannot do that during open atm (m3fs
        // already borrowed as mutable). it's the wrong semantic anyway, so ...
        wv_assert_ok!(file.seek(0, SeekMode::End));

        let buf = _get_pat_vector(1024);
        for _ in 0..2 {
            wv_assert_eq!(t, file.write_all(&buf[0..1024]), Ok(()));
        }
    }

    _validate_pattern_file(t, "/test.txt", 1024 * 4);
}

fn append_read(t: &mut dyn WvTester) {
    {
        let mut file = wv_assert_ok!(VFS::open(
            "/test.txt",
            OpenFlags::RW | OpenFlags::TRUNC | OpenFlags::CREATE
        ));

        let pat = _get_pat_vector(1024);
        for _ in 0..2 {
            wv_assert_eq!(t, file.write_all(&pat[0..1024]), Ok(()));
        }

        // there is nothing to read now
        let mut buf = [0u8; 1024];
        wv_assert_eq!(t, file.read(&mut buf), Ok(0));

        // seek back
        wv_assert_eq!(t, file.seek(1 * 1024, SeekMode::Set), Ok(1 * 1024));

        // now reading should work
        wv_assert_eq!(t, file.read(&mut buf), Ok(1024));

        // seek back and overwrite
        wv_assert_eq!(t, file.seek(2 * 1024, SeekMode::Set), Ok(2 * 1024));

        for _ in 0..2 {
            wv_assert_eq!(t, file.write_all(&pat[0..1024]), Ok(()));
        }
    }

    _validate_pattern_file(t, "/test.txt", 1024 * 4);
}

fn _get_pat_vector(size: usize) -> Vec<u8> {
    let mut buf = Vec::with_capacity(size);
    for i in 0..1024 {
        buf.push(i as u8)
    }
    buf
}

fn _validate_pattern_file(t: &mut dyn WvTester, filename: &str, size: usize) {
    let mut file = wv_assert_ok!(VFS::open(filename, OpenFlags::R));

    let info = wv_assert_ok!(file.stat());
    wv_assert_eq!(t, { info.size }, size);

    let mut buf = [0u8; 1024];
    wv_assert_eq!(t, _validate_pattern_content(t, &mut file, &mut buf), size);
}

fn _validate_pattern_content(
    t: &mut dyn WvTester,
    file: &mut FileRef<GenericFile>,
    buf: &mut [u8],
) -> usize {
    let mut pos: usize = 0;
    loop {
        let count = wv_assert_ok!(file.read(buf));
        if count == 0 {
            break;
        }

        for b in buf.iter().take(count) {
            wv_assert_eq!(
                t,
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
