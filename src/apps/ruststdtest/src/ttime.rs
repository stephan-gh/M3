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

use m3::test::WvTester;
use m3::{wv_assert, wv_run_test};

use std::thread::sleep;
use std::time::{Duration, Instant};

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, basics);
}

fn basics(t: &mut dyn WvTester) {
    let instant = Instant::now();
    let three_millis = Duration::from_millis(3);
    sleep(three_millis);
    wv_assert!(t, instant.elapsed() >= three_millis);
}
