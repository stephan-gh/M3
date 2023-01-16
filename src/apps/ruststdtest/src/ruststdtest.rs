/*
 * Copyright (C) 2022 Nils Asmussen, Barkhausen Institut
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

#![feature(io_error_more)]

extern crate m3impl as m3;

use m3::errors::Error;
use m3::test::{DefaultWvTester, WvTester};
use m3::wv_run_suite;

mod tdir;
mod tfile;
mod tsocket;
mod ttime;

#[macro_export]
macro_rules! wv_assert_stderr {
    ($t:expr, $a:expr, $e:expr) => {{
        m3::wv_assert!($t, matches!($a, Err(e) if e.kind() == $e));
    }};
}

#[no_mangle]
pub fn main() -> Result<(), Error> {
    let mut tester = DefaultWvTester::default();
    wv_run_suite!(tester, tdir::run);
    wv_run_suite!(tester, tfile::run);
    wv_run_suite!(tester, tsocket::run);
    wv_run_suite!(tester, ttime::run);
    println!("{}", tester);
    Ok(())
}
