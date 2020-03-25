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

use m3::profile;
use m3::tcu::TCUIf;
use m3::test;

pub fn run(t: &mut dyn test::WvTester) {
    wv_run_test!(t, pexcalls);
}

fn pexcalls() {
    let mut prof = profile::Profiler::default().repeats(100).warmup(30);
    wv_perf!(
        "noop pexcall",
        prof.run_with_id(|| TCUIf::noop().unwrap(), 0x30)
    );
}
