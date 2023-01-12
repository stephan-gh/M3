/*
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

use m3::cap::Selector;
use m3::com::Semaphore;
use m3::io::{Read, Write};
use m3::test::{DefaultWvTester, WvTester};
use m3::tiles::{Activity, ChildActivity, RunningActivity, Tile};
use m3::vfs::{OpenFlags, VFS};
use m3::{wv_assert_eq, wv_assert_ok, wv_run_test};

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, taking_turns);
}

fn get_counter(filename: &str) -> u32 {
    let mut file = wv_assert_ok!(VFS::open(filename, OpenFlags::R));

    let buf = wv_assert_ok!(file.read_to_string());
    buf.parse::<u32>().unwrap()
}

fn set_counter(filename: &str, value: u32) {
    let mut file = wv_assert_ok!(VFS::open(
        filename,
        OpenFlags::W | OpenFlags::TRUNC | OpenFlags::CREATE
    ));
    wv_assert_ok!(write!(file, "{}", value));
}

fn taking_turns(t: &mut dyn WvTester) {
    let sem0 = wv_assert_ok!(Semaphore::create(1));
    let sem1 = wv_assert_ok!(Semaphore::create(0));

    let tile = wv_assert_ok!(Tile::get("compat|own"));
    let mut child = wv_assert_ok!(ChildActivity::new(tile, "child"));
    wv_assert_ok!(child.delegate_obj(sem0.sel()));
    wv_assert_ok!(child.delegate_obj(sem1.sel()));

    child.add_mount("/", "/");

    set_counter("/sem0", 0);
    set_counter("/sem1", 0);

    let mut dst = child.data_sink();
    dst.push(sem0.sel());
    dst.push(sem1.sel());

    let act = wv_assert_ok!(child.run(|| {
        let mut t = DefaultWvTester::default();
        let mut src = Activity::own().data_source();
        let sem0_sel: Selector = src.pop().unwrap();
        let sem1_sel: Selector = src.pop().unwrap();

        let sem0 = Semaphore::bind(sem0_sel);
        let sem1 = Semaphore::bind(sem1_sel);
        for i in 0..10 {
            wv_assert_ok!(sem0.down());
            wv_assert_eq!(t, get_counter("/sem0"), i);
            set_counter("/sem1", i);
            wv_assert_ok!(sem1.up());
        }
        Ok(())
    }));

    for i in 0..10 {
        wv_assert_ok!(sem1.down());
        wv_assert_eq!(t, get_counter("/sem1"), i);
        set_counter("/sem0", i + 1);
        wv_assert_ok!(sem0.up());
    }

    wv_assert_ok!(act.wait());
}
