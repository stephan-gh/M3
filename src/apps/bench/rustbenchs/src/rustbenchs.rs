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
#![feature(core_intrinsics)]

#[macro_use]
extern crate m3;

mod bboxlist;
mod bdlist;
mod bmemmap;
mod bmgate;
mod bpipe;
mod bregfile;
mod bstream;
mod bsyscall;
mod btreap;
mod btreemap;

use m3::mem::heap;
use m3::test::WvTester;
use m3::vfs::VFS;
use m3::vpe::VPE;

struct MyTester {
}

impl WvTester for MyTester {
    fn run_suite(&mut self, name: &str, f: &dyn Fn(&mut dyn WvTester)) {
        println!("Running benchmark suite {} ...\n", name);
        f(self);
        println!("");
    }

    fn run_test(&mut self, name: &str, file: &str, f: &dyn Fn()) {
        println!("Testing \"{}\" in {}:", name, file);
        let free_mem = heap::free_memory();
        f();
        wv_assert_eq!(heap::free_memory(), free_mem);
        println!("");
    }
}

#[no_mangle]
pub fn main() -> i32 {
    // pass one EP caps to m3fs (required for OpenFlags::NOSESS)
    // do that here to prevent that the heap-free-memory-check above fails
    let ep = VPE::cur().alloc_ep()
        .expect("Unable to allocate EP for meta session");
    VFS::delegate_eps("/", VPE::cur().ep_sel(ep), 1)
        .expect("Unable to delegate EPs to meta session");

    let mut tester = MyTester {};
    wv_run_suite!(tester, bboxlist::run);
    wv_run_suite!(tester, bdlist::run);
    wv_run_suite!(tester, bmemmap::run);
    wv_run_suite!(tester, bmgate::run);
    wv_run_suite!(tester, bpipe::run);
    wv_run_suite!(tester, bregfile::run);
    wv_run_suite!(tester, bstream::run);
    wv_run_suite!(tester, bsyscall::run);
    wv_run_suite!(tester, btreap::run);
    wv_run_suite!(tester, btreemap::run);

    println!("\x1B[1;32mAll tests successful!\x1B[0;m");
    0
}
