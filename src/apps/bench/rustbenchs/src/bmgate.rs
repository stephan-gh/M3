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

use m3::cell::StaticCell;
use m3::com::MemGate;
use m3::kif;
use m3::mem::AlignedBuf;
use m3::profile;
use m3::test;
use m3::{wv_perf, wv_run_test};

const SIZE: usize = 2 * 1024 * 1024;

static BUF: StaticCell<AlignedBuf<{ 8192 + 64 }>> = StaticCell::new(AlignedBuf::new_zeroed());

pub fn run(t: &mut dyn test::WvTester) {
    wv_run_test!(t, read);
    wv_run_test!(t, read_unaligned);
    wv_run_test!(t, write);
    wv_run_test!(t, write_unaligned);
}

fn read() {
    let mut buf = &mut BUF.get_mut()[..8192];
    let mgate = MemGate::new(8192, kif::Perm::R).expect("Unable to create mgate");

    let mut prof = profile::Profiler::default().repeats(2).warmup(1);

    wv_perf!(
        "read 2 MiB with 8K buf",
        prof.run_with_id(
            || {
                let mut total = 0;
                while total < SIZE {
                    mgate.read(&mut buf, 0).expect("Reading failed");
                    total += buf.len();
                }
            },
            0x30
        )
    );
}

fn read_unaligned() {
    let mut buf = &mut BUF.get_mut()[64..];
    let mgate = MemGate::new(8192, kif::Perm::R).expect("Unable to create mgate");

    let mut prof = profile::Profiler::default().repeats(2).warmup(1);

    wv_perf!(
        "read unaligned 2 MiB with 8K buf",
        prof.run_with_id(
            || {
                let mut total = 0;
                while total < SIZE {
                    mgate.read(&mut buf, 0).expect("Reading failed");
                    total += buf.len();
                }
            },
            0x30
        )
    );
}

fn write() {
    let buf = &BUF[..8192];
    let mgate = MemGate::new(8192, kif::Perm::W).expect("Unable to create mgate");

    let mut prof = profile::Profiler::default().repeats(2).warmup(1);

    wv_perf!(
        "write 2 MiB with 8K buf",
        prof.run_with_id(
            || {
                let mut total = 0;
                while total < SIZE {
                    mgate.write(&buf, 0).expect("Writing failed");
                    total += buf.len();
                }
            },
            0x31
        )
    );
}

fn write_unaligned() {
    let buf = &BUF[64..];
    let mgate = MemGate::new(8192, kif::Perm::W).expect("Unable to create mgate");

    let mut prof = profile::Profiler::default().repeats(2).warmup(1);

    wv_perf!(
        "write unaligned 2 MiB with 8K buf",
        prof.run_with_id(
            || {
                let mut total = 0;
                while total < SIZE {
                    mgate.write(&buf, 0).expect("Writing failed");
                    total += buf.len();
                }
            },
            0x31
        )
    );
}
