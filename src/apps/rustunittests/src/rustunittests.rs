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

#![no_std]

#[macro_use]
extern crate m3;

use m3::mem::heap;
use m3::test::WvTester;
use m3::vfs::VFS;

mod tboxlist;
mod tbufio;
mod tdir;
mod tdlist;
mod tfilemux;
mod tgenfile;
mod tm3fs;
mod tmemmap;
mod tmgate;
mod tpipe;
mod trgate;
mod tsems;
mod tserver;
mod tsgate;
mod tsyscalls;
mod ttreap;
mod tvpe;

struct MyTester {
}

impl WvTester for MyTester {
    fn run_suite(&mut self, name: &str, f: &dyn Fn(&mut dyn WvTester)) {
        println!("Running test suite {} ...\n", name);
        f(self);
        println!();
    }

    fn run_test(&mut self, name: &str, file: &str, f: &dyn Fn()) {
        println!("Testing \"{}\" in {}:", name, file);
        let free_mem = heap::free_memory();
        f();
        wv_assert_eq!(heap::free_memory(), free_mem);
        println!();
    }
}

#[no_mangle]
pub fn main() -> i32 {
    // do a mount here to ensure that we don't need to realloc the mount-table later, which screws
    // up our simple memory-leak detection above
    wv_assert_ok!(VFS::mount("/fs/", "m3fs", "m3fs-clone"));
    wv_assert_ok!(VFS::unmount("/fs/"));

    let mut tester = MyTester {};
    wv_run_suite!(tester, tboxlist::run);
    wv_run_suite!(tester, tbufio::run);
    wv_run_suite!(tester, tdir::run);
    wv_run_suite!(tester, tdlist::run);
    wv_run_suite!(tester, tfilemux::run);
    wv_run_suite!(tester, tgenfile::run);
    wv_run_suite!(tester, tm3fs::run);
    wv_run_suite!(tester, tmemmap::run);
    wv_run_suite!(tester, tmgate::run);
    wv_run_suite!(tester, tpipe::run);
    wv_run_suite!(tester, trgate::run);
    wv_run_suite!(tester, tsgate::run);
    wv_run_suite!(tester, tsems::run);
    wv_run_suite!(tester, tserver::run);
    wv_run_suite!(tester, tsyscalls::run);
    wv_run_suite!(tester, ttreap::run);
    wv_run_suite!(tester, tvpe::run);

    println!("\x1B[1;32mAll tests successful!\x1B[0;m");
    0
}
