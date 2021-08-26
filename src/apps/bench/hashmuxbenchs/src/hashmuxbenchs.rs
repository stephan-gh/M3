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

mod bclients;
mod bhash;
mod util;

use m3::col::Vec;
use m3::test::WvTester;
use m3::{println, wv_run_suite};

struct MyTester {
    suites: Vec<&'static str>,
}

impl WvTester for MyTester {
    fn run_suite(&mut self, name: &str, f: &dyn Fn(&mut dyn WvTester)) {
        if !self.suites.is_empty() && !self.suites.iter().any(|&s| name.starts_with(s)) {
            println!("Skipping benchmark suite {}", name);
            return;
        }

        println!("Running benchmark suite {} ...\n", name);
        f(self);
        println!();
    }

    fn run_test(&mut self, name: &str, file: &str, f: &dyn Fn()) {
        println!("Testing \"{}\" in {}:", name, file);
        f();
        println!();
    }
}

#[no_mangle]
pub fn main() -> i32 {
    let mut tester = MyTester {
        suites: m3::env::args().skip(1).collect(), // Skip program name
    };
    wv_run_suite!(tester, bhash::run);
    wv_run_suite!(tester, bclients::run);
    0
}
