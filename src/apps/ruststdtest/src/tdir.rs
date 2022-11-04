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

use m3::test::WvTester;
use m3::{wv_assert, wv_assert_eq, wv_assert_ok, wv_assert_some, wv_run_test};

use std::fs::{self, File};
use std::io::{ErrorKind, Write};

use crate::wv_assert_stderr;

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, mkdir_rmdir);
    wv_run_test!(t, rename);
    wv_run_test!(t, listing);
    wv_run_test!(t, stat);
}

fn mkdir_rmdir(t: &mut dyn WvTester) {
    wv_assert_ok!(fs::create_dir("/tmp/foo"));
    wv_assert_stderr!(t, fs::create_dir("/tmp/foo"), ErrorKind::AlreadyExists);

    {
        let mut file = wv_assert_ok!(File::create("/tmp/foo/myfile.txt"));
        wv_assert!(t, matches!(file.write(b"test"), Ok(4)));
    }

    wv_assert_stderr!(t, fs::remove_dir("/tmp/foo"), ErrorKind::DirectoryNotEmpty);
    wv_assert_ok!(fs::remove_file("/tmp/foo/myfile.txt"));
    wv_assert_ok!(fs::remove_dir("/tmp/foo"));
    wv_assert_stderr!(t, fs::remove_dir("/tmp/foo"), ErrorKind::NotFound);
}

fn rename(t: &mut dyn WvTester) {
    wv_assert_ok!(File::create("/tmp/myfile.txt"));

    wv_assert_ok!(fs::rename("/tmp/myfile.txt", "/tmp/yourfile.txt"));
    wv_assert_stderr!(t, fs::remove_file("/tmp/myfile.txt"), ErrorKind::NotFound);
    wv_assert_ok!(fs::remove_file("/tmp/yourfile.txt"));
}

fn listing(t: &mut dyn WvTester) {
    let mut entries = Vec::new();
    for e in wv_assert_ok!(fs::read_dir("/largedir")) {
        let file_name = e.unwrap().file_name().into_string().unwrap();
        let no_str = wv_assert_some!(file_name.split('.').next());
        let no = wv_assert_ok!(no_str.parse::<usize>());
        entries.push(no);
    }

    wv_assert_eq!(t, entries.len(), 80);
    entries.sort();
    for (i, entry) in entries.iter().enumerate().take(80) {
        wv_assert_eq!(t, *entry, i);
    }
}

fn stat(t: &mut dyn WvTester) {
    {
        let meta = wv_assert_ok!(fs::metadata("/test.txt"));
        wv_assert_eq!(t, meta.len(), 15);
    }

    {
        let file = wv_assert_ok!(File::open("/test.txt"));
        let meta = wv_assert_ok!(file.metadata());
        wv_assert_eq!(t, meta.len(), 15);
    }
}
