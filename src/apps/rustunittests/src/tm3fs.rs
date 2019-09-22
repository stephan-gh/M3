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

use m3::errors::Code;
use m3::io::Write;
use m3::test;
use m3::vfs::{OpenFlags, VFS};

pub fn run(t: &mut dyn test::WvTester) {
    wv_run_test!(t, meta_ops);
}

#[allow(clippy::cognitive_complexity)]
pub fn meta_ops() {
    wv_assert_ok!(VFS::mkdir("/example", 0o755));
    wv_assert_err!(VFS::mkdir("/example", 0o755), Code::Exists);
    wv_assert_err!(VFS::mkdir("/example/foo/bar", 0o755), Code::NoSuchFile);

    {
        let mut file = wv_assert_ok!(VFS::open(
            "/example/myfile",
            OpenFlags::W | OpenFlags::CREATE
        ));
        wv_assert_ok!(write!(file, "text\n"));
    }

    {
        wv_assert_ok!(VFS::mount("/fs/", "m3fs", "m3fs-clone"));
        wv_assert_err!(VFS::link("/example/myfile", "/fs/foo"), Code::XfsLink);
        wv_assert_ok!(VFS::unmount("/fs/"));
    }

    wv_assert_err!(VFS::rmdir("/example/foo/bar"), Code::NoSuchFile);
    wv_assert_err!(VFS::rmdir("/example/myfile"), Code::IsNoDir);
    wv_assert_err!(VFS::rmdir("/example"), Code::DirNotEmpty);

    wv_assert_err!(VFS::link("/example", "/newpath"), Code::IsDir);
    wv_assert_ok!(VFS::link("/example/myfile", "/newpath"));

    wv_assert_err!(VFS::unlink("/example"), Code::IsDir);
    wv_assert_err!(VFS::unlink("/example/foo"), Code::NoSuchFile);
    wv_assert_ok!(VFS::unlink("/example/myfile"));

    wv_assert_ok!(VFS::rmdir("/example"));

    wv_assert_ok!(VFS::unlink("/newpath"));
}
