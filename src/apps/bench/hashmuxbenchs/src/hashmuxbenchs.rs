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

mod bclients;
mod bhash;
mod blatency;
mod util;

use m3::col::Vec;
use m3::errors::Error;
use m3::test::{DefaultWvTester, WvTester};
use m3::{println, wv_run_suite};

struct MyTester {
    def: DefaultWvTester,
    suites: Vec<&'static str>,
}

impl WvTester for MyTester {
    fn run_suite(&mut self, name: &str, f: &dyn Fn(&mut dyn WvTester)) {
        if !self.suites.is_empty() && !self.suites.iter().any(|&s| name.starts_with(s)) {
            println!("Skipping benchmark suite {}", name);
            return;
        }

        self.def.run_suite(name, f);
    }

    fn run_test(&mut self, name: &str, file: &str, f: &dyn Fn(&mut dyn WvTester)) {
        self.def.run_test(name, file, f);
    }

    fn test_succeeded(&mut self) {
        self.def.test_succeeded();
    }

    fn test_failed(&mut self) {
        self.def.test_failed();
    }
}

#[no_mangle]
pub fn main() -> Result<(), Error> {
    let mut tester = MyTester {
        def: DefaultWvTester::default(),
        suites: m3::env::args().skip(1).collect(), // Skip program name
    };
    wv_run_suite!(tester, bhash::run);
    wv_run_suite!(tester, bclients::run);
    wv_run_suite!(tester, blatency::run);
    Ok(())
}
