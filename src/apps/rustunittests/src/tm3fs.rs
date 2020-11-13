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
use m3::{wv_assert_err, wv_assert_ok, wv_run_test};

pub fn run(t: &mut dyn test::WvTester) {
    wv_run_test!(t, mkdir_rmdir);
    wv_run_test!(t, link_unlink);
    wv_run_test!(t, rename);
}

fn setup() {
    wv_assert_ok!(VFS::mkdir("/example", 0o755));

    let mut file = wv_assert_ok!(VFS::open(
        "/example/myfile",
        OpenFlags::W | OpenFlags::CREATE
    ));
    wv_assert_ok!(write!(file, "text\n"));
}

fn teardown() {
    wv_assert_ok!(VFS::unlink("/example/myfile/"));
    wv_assert_ok!(VFS::rmdir("/example/"));
}

fn mkdir_rmdir() {
    setup();

    // create and remove directory within directory
    wv_assert_ok!(VFS::mkdir("/parent", 0o755));
    wv_assert_ok!(VFS::mkdir("/parent/child", 0o755));
    wv_assert_ok!(VFS::rmdir("/parent/child"));
    wv_assert_ok!(VFS::rmdir("/parent"));

    // use weird paths
    wv_assert_err!(VFS::mkdir("/foo/.", 0o755), Code::NoSuchFile);
    wv_assert_err!(VFS::mkdir("/foo/..", 0o755), Code::NoSuchFile);
    wv_assert_ok!(VFS::mkdir("/./../foo/", 0o755));
    wv_assert_err!(VFS::rmdir("/foo/."), Code::InvArgs);
    wv_assert_err!(VFS::rmdir("/foo/bar/.."), Code::NoSuchFile);
    wv_assert_ok!(VFS::rmdir("///.././foo///"));

    // test mkdir errors
    wv_assert_err!(VFS::mkdir("/", 0o755), Code::Exists);
    wv_assert_err!(VFS::mkdir("/example", 0o755), Code::Exists);
    wv_assert_err!(VFS::mkdir("/example/foo/bar", 0o755), Code::NoSuchFile);

    // test rmdir errors
    wv_assert_err!(VFS::rmdir("/example/foo/bar"), Code::NoSuchFile);
    wv_assert_err!(VFS::rmdir("/example/myfile/"), Code::IsNoDir);
    wv_assert_err!(VFS::rmdir("/example"), Code::DirNotEmpty);
    wv_assert_err!(VFS::rmdir("/"), Code::InvArgs);

    teardown();
}

fn link_unlink() {
    setup();

    // test link errors
    wv_assert_err!(VFS::link("/example/", "/"), Code::IsDir);
    wv_assert_err!(VFS::link("/example", "/newpath"), Code::IsDir);
    wv_assert_ok!(VFS::link("/example/myfile/", "/newpath"));
    wv_assert_err!(VFS::link("/example/myfile", "/newpath"), Code::Exists);

    // use weird paths
    wv_assert_err!(VFS::link("/example/myfile/.", "/newtest"), Code::IsNoDir);
    wv_assert_err!(VFS::link("/example/myfile/..", "/newtest"), Code::IsNoDir);
    wv_assert_err!(VFS::link("/example/myfile", "/newtest/."), Code::NoSuchFile);
    wv_assert_err!(
        VFS::link("/example/myfile", "/newtest/.."),
        Code::NoSuchFile
    );
    wv_assert_ok!(VFS::link("//example/./../example/myfile", "/newtest"));
    wv_assert_err!(VFS::unlink("/example/myfile/."), Code::InvArgs);
    wv_assert_err!(VFS::unlink("/example/myfile/.."), Code::InvArgs);
    wv_assert_ok!(VFS::unlink("///example///../newtest"));

    // test unlink errors
    wv_assert_err!(VFS::unlink("/"), Code::InvArgs);
    wv_assert_err!(VFS::unlink("/example//"), Code::IsDir);
    wv_assert_err!(VFS::unlink("/example/foo"), Code::NoSuchFile);

    // test cross-fs link
    wv_assert_ok!(VFS::mount("/fs/", "m3fs", "m3fs-clone"));
    wv_assert_err!(VFS::link("/example/myfile", "/fs/foo"), Code::XfsLink);
    wv_assert_ok!(VFS::unmount("/fs/"));

    teardown();
}

fn rename() {
    setup();

    // test errors
    wv_assert_err!(VFS::rename("/", "/example"), Code::InvArgs);
    wv_assert_err!(VFS::rename("/example/myfile", "/"), Code::InvArgs);
    wv_assert_err!(VFS::rename("/example", "/example"), Code::IsDir);
    wv_assert_err!(
        VFS::rename("/example/myfiles", "/example/myfile2"),
        Code::NoSuchFile
    );

    // use weird paths
    wv_assert_err!(
        VFS::rename("/example/myfile/.", "/example/myotherfile"),
        Code::InvArgs
    );
    wv_assert_err!(
        VFS::rename("/example/myfile/..", "/example/myotherfile"),
        Code::InvArgs
    );
    wv_assert_err!(
        VFS::rename("/example/myfile", "/example/myotherfile/."),
        Code::InvArgs
    );
    wv_assert_err!(
        VFS::rename("/example/myfile", "/example/myotherfile/.."),
        Code::InvArgs
    );
    wv_assert_err!(
        VFS::rename("/example/myfile/bar", "/example/myotherfile"),
        Code::NoSuchFile
    );

    // successful rename
    wv_assert_ok!(VFS::rename(
        "//example/./myfile",
        "/example/../example/myotherfile//"
    ));
    wv_assert_err!(VFS::open("/example/myfile", OpenFlags::R), Code::NoSuchFile);
    wv_assert_ok!(VFS::open("/example/myotherfile", OpenFlags::R));

    // if both link to the same file, rename has no effect
    wv_assert_ok!(VFS::link("/example/myotherfile", "/example/myotherfile2"));
    wv_assert_ok!(VFS::rename("/example/myotherfile", "/example/myotherfile2"));
    wv_assert_ok!(VFS::open("/example/myotherfile", OpenFlags::R));
    wv_assert_ok!(VFS::open("/example/myotherfile2", OpenFlags::R));

    // undo changes
    wv_assert_ok!(VFS::unlink("/example/myotherfile"));
    wv_assert_ok!(VFS::rename("/example/myotherfile2", "/example/myfile"));

    teardown();
}
