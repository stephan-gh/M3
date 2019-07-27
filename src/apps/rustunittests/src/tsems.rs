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

use m3::boxed::Box;
use m3::col::String;
use m3::com::Semaphore;
use m3::io::{Read, Write};
use m3::test;
use m3::vfs::{OpenFlags, VFS};
use m3::vpe::{Activity, VPE};

pub fn run(t: &mut dyn test::WvTester) {
    wv_run_test!(t, taking_turns);
}

fn get_counter(filename: &str) -> u32 {
    let mut file = wv_assert_ok!(VFS::open(filename, OpenFlags::R));

    let mut buf = String::new();
    wv_assert_ok!(file.read_to_string(&mut buf));
    buf.parse::<u32>().unwrap()
}

fn set_counter(filename: &str, value: u32) {
    let mut file = wv_assert_ok!(VFS::open(filename,
                                        OpenFlags::W | OpenFlags::TRUNC | OpenFlags::CREATE));
    wv_assert_ok!(write!(file, "{}", value));
}

fn taking_turns() {
    let sem0 = wv_assert_ok!(Semaphore::create(1));
    let sem1 = wv_assert_ok!(Semaphore::create(0));

    let mut child = wv_assert_ok!(VPE::new("child"));
    wv_assert_ok!(child.delegate_obj(sem0.sel()));
    wv_assert_ok!(child.delegate_obj(sem1.sel()));

    let rootmnt = wv_assert_some!(VPE::cur().mounts().get_by_path("/"));
    wv_assert_ok!(child.mounts().add("/", rootmnt));
    wv_assert_ok!(child.obtain_mounts());

    set_counter("/sem0", 0);
    set_counter("/sem1", 0);

    let sem0_sel = sem0.sel();
    let sem1_sel = sem1.sel();

    let act = wv_assert_ok!(child.run(Box::new(move || {
        let sem0 = Semaphore::bind(sem0_sel);
        let sem1 = Semaphore::bind(sem1_sel);
        for i in 0..10 {
            wv_assert_ok!(sem0.down());
            wv_assert_eq!(get_counter("/sem0"), i);
            set_counter("/sem1", i);
            wv_assert_ok!(sem1.up());
        }
        0
    })));

    for i in 0..10 {
        wv_assert_ok!(sem1.down());
        wv_assert_eq!(get_counter("/sem1"), i);
        set_counter("/sem0", i + 1);
        wv_assert_ok!(sem0.up());
    }

    wv_assert_ok!(act.wait());
}
