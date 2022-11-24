/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
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

#![no_std]

mod bboxlist;
mod bdlist;
mod bipc;
mod bmemmap;
mod bmgate;
mod bpipe;
mod bregfile;
mod bstream;
mod bsyscall;
mod btilemux;
mod btreap;
mod btreemap;

use m3::errors::Error;
use m3::test::{DefaultWvTester, WvTester};
use m3::{println, wv_run_suite};

#[no_mangle]
pub fn main() -> Result<(), Error> {
    let mut tester = DefaultWvTester::default();
    wv_run_suite!(tester, bboxlist::run);
    wv_run_suite!(tester, bdlist::run);
    wv_run_suite!(tester, bmemmap::run);
    wv_run_suite!(tester, bmgate::run);
    wv_run_suite!(tester, bipc::run);
    wv_run_suite!(tester, btilemux::run);
    wv_run_suite!(tester, bpipe::run);
    wv_run_suite!(tester, bregfile::run);
    wv_run_suite!(tester, bstream::run);
    wv_run_suite!(tester, bsyscall::run);
    wv_run_suite!(tester, btreap::run);
    wv_run_suite!(tester, btreemap::run);
    println!("{}", tester);
    Ok(())
}
