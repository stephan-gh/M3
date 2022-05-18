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

#![no_std]
#![feature(core_intrinsics)]

#[path = "../../rustbenchs/src/bipc.rs"]
mod bipc;

use m3::test::{DefaultWvTester, WvTester};
use m3::{println, wv_run_suite};

#[no_mangle]
pub fn main() -> i32 {
    let mut tester = DefaultWvTester::default();
    wv_run_suite!(tester, bipc::run);

    if tester.failures() > 0 {
        println!(
            "\x1B[1;31m{} of {} tests failed\x1B[0;m",
            tester.failures(),
            tester.tests()
        );
    }
    else {
        println!("\x1B[1;32mAll tests successful!\x1B[0;m");
    }
    0
}
