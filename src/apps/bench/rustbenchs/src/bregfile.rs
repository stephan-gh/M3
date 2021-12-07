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

use m3::cell::StaticRefCell;
use m3::io::Read;
use m3::mem::AlignedBuf;
use m3::profile;
use m3::test;
use m3::time::CycleInstant;
use m3::vfs::{OpenFlags, VFS};
use m3::{wv_assert_ok, wv_perf, wv_run_test};

static BUF: StaticRefCell<AlignedBuf<8192>> = StaticRefCell::new(AlignedBuf::new_zeroed());

pub fn run(t: &mut dyn test::WvTester) {
    wv_run_test!(t, open_close);
    wv_run_test!(t, stat);
    wv_run_test!(t, mkdir_rmdir);
    wv_run_test!(t, link_unlink);
    wv_run_test!(t, read);
    wv_run_test!(t, write);
    wv_run_test!(t, copy);
}

fn open_close() {
    let mut prof = profile::Profiler::default().repeats(50).warmup(10);

    wv_perf!(
        "open-close",
        prof.run::<CycleInstant, _>(|| {
            wv_assert_ok!(VFS::open("/data/2048k.txt", OpenFlags::R));
        })
    );
}

fn stat() {
    let mut prof = profile::Profiler::default().repeats(50).warmup(10);

    wv_perf!(
        "stat",
        prof.run::<CycleInstant, _>(|| {
            wv_assert_ok!(VFS::stat("/data/2048k.txt"));
        })
    );
}

fn mkdir_rmdir() {
    let mut prof = profile::Profiler::default().repeats(50).warmup(10);

    wv_perf!(
        "mkdir_rmdir",
        prof.run::<CycleInstant, _>(|| {
            wv_assert_ok!(VFS::mkdir("/newdir", 0o755));
            wv_assert_ok!(VFS::rmdir("/newdir"));
        })
    );
}

fn link_unlink() {
    let mut prof = profile::Profiler::default().repeats(50).warmup(10);

    wv_perf!(
        "link_unlink",
        prof.run::<CycleInstant, _>(|| {
            wv_assert_ok!(VFS::link("/large.txt", "/newlarge.txt"));
            wv_assert_ok!(VFS::unlink("/newlarge.txt"));
        })
    );
}

fn read() {
    let mut buf = &mut BUF.borrow_mut()[..];

    let mut prof = profile::Profiler::default().repeats(2).warmup(1);

    wv_perf!(
        "read 2 MiB file with 8K buf",
        prof.run::<CycleInstant, _>(|| {
            let mut file = wv_assert_ok!(VFS::open("/data/2048k.txt", OpenFlags::R));
            loop {
                let amount = wv_assert_ok!(file.read(&mut buf));
                if amount == 0 {
                    break;
                }
            }
        })
    );
}

fn write() {
    const SIZE: usize = 2 * 1024 * 1024;
    let buf = &BUF.borrow()[..];

    let mut prof = profile::Profiler::default().repeats(2).warmup(1);

    wv_perf!(
        "write 2 MiB file with 8K buf",
        prof.run::<CycleInstant, _>(|| {
            let mut file = wv_assert_ok!(VFS::open(
                "/newfile",
                OpenFlags::W | OpenFlags::CREATE | OpenFlags::TRUNC
            ));

            let mut total = 0;
            while total < SIZE {
                let amount = wv_assert_ok!(file.write(&buf));
                if amount == 0 {
                    break;
                }
                total += amount;
            }
        })
    );
}

fn copy() {
    let mut buf = &mut BUF.borrow_mut()[..];
    let mut prof = profile::Profiler::default().repeats(2).warmup(1);

    wv_perf!(
        "copy 2 MiB file with 8K buf",
        prof.run::<CycleInstant, _>(|| {
            let mut fin = wv_assert_ok!(VFS::open("/data/2048k.txt", OpenFlags::R));
            let mut fout = wv_assert_ok!(VFS::open(
                "/newfile",
                OpenFlags::W | OpenFlags::CREATE | OpenFlags::TRUNC
            ));

            loop {
                let amount = wv_assert_ok!(fin.read(&mut buf));
                if amount == 0 {
                    break;
                }
                wv_assert_ok!(fout.write_all(&buf[0..amount]));
            }
        })
    );
}
