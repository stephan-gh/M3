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
use m3::{wv_assert, wv_assert_eq, wv_assert_ok, wv_run_test};

use std::fs::{self, File};
use std::io::{ErrorKind, Read, Seek, Write};
use std::path::Path;

use crate::wv_assert_stderr;

static TEST_CONTENT: [u8; 15] = *b"This is a test\n";
static TEST_CONTENT_TWICE: [u8; 30] = *b"This is a test\nThis is a test\n";

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, basics);
    wv_run_test!(t, misc);
}

fn basics(t: &mut dyn WvTester) {
    {
        let mut file = wv_assert_ok!(File::open("test.txt"));
        wv_assert_stderr!(t, file.write(b"foo"), ErrorKind::PermissionDenied);
        wv_assert!(t, matches!(file.read(&mut [0u8; 1]), Ok(1)));
    }

    {
        let mut file = wv_assert_ok!(File::options().write(true).open("test.txt"));
        wv_assert_stderr!(t, file.read(&mut [0u8; 1]), ErrorKind::PermissionDenied);
        wv_assert!(t, matches!(file.write(&TEST_CONTENT[..1]), Ok(1)));
    }

    {
        let mut file = wv_assert_ok!(File::options().read(true).write(true).open("test.txt"));
        let mut buf = [0u8; 1];
        wv_assert!(t, matches!(file.read(&mut buf), Ok(1)));
        wv_assert_ok!(file.rewind());
        wv_assert!(t, matches!(file.write(&buf), Ok(1)));
    }

    {
        let mut file = wv_assert_ok!(File::options()
            .read(true)
            .write(true)
            .append(true)
            .open("test.txt"));
        let mut buf = [0u8; 30];
        wv_assert!(t, matches!(file.write(&TEST_CONTENT), Ok(15)));
        wv_assert_ok!(file.rewind());
        wv_assert!(t, matches!(file.read(&mut buf), Ok(30)));
        wv_assert_eq!(t, buf, TEST_CONTENT_TWICE);
        wv_assert_ok!(file.set_len(20));
        wv_assert_ok!(file.rewind());
        wv_assert!(t, matches!(file.read(&mut buf), Ok(20)));
        wv_assert_eq!(t, buf[..20], TEST_CONTENT_TWICE[..20]);
    }

    {
        let mut file = wv_assert_ok!(File::options()
            .read(true)
            .write(true)
            .truncate(true)
            .open("test.txt"));
        let mut buf = [0u8; 15];
        wv_assert!(t, matches!(file.write(&TEST_CONTENT), Ok(15)));
        wv_assert_ok!(file.rewind());
        wv_assert!(t, matches!(file.read(&mut buf), Ok(15)));
        wv_assert_eq!(t, buf, TEST_CONTENT);
    }

    {
        wv_assert_ok!(File::create("/tmp/test.txt"));
        wv_assert_ok!(File::open("/tmp/test.txt"));
        wv_assert_ok!(fs::remove_file("/tmp/test.txt"));
    }
}

fn misc(t: &mut dyn WvTester) {
    {
        let file = wv_assert_ok!(File::open("test.txt"));
        wv_assert_ok!(file.sync_data());
        wv_assert_ok!(file.sync_all());
        let meta = wv_assert_ok!(file.metadata());
        wv_assert_eq!(t, meta.len(), 15);
        wv_assert!(t, meta.file_type().is_file());
    }

    let abs = wv_assert_ok!(fs::canonicalize("../bin/../test.txt"));
    wv_assert_eq!(t, abs, Path::new("/test.txt"));
}
