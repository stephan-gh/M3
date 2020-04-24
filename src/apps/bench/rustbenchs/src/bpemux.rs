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

use core::fmt;
use m3::cfg;
use m3::com::MemGate;
use m3::goff;
use m3::kif::Perm;
use m3::pes::VPE;
use m3::profile;
use m3::session::MapFlags;
use m3::tcu::TCUIf;
use m3::test;
use m3::time;

pub fn run(t: &mut dyn test::WvTester) {
    wv_run_test!(t, pexcalls);
    wv_run_test!(t, translates);
}

fn pexcalls() {
    let mut prof = profile::Profiler::default().repeats(100).warmup(30);
    wv_perf!(
        "noop pexcall",
        prof.run_with_id(|| TCUIf::noop().unwrap(), 0x30)
    );
}

fn translates() {
    if VPE::cur().pager().is_none() {
        println!("PE has no virtual memory support; skipping translate benchmark");
        return;
    }

    const VIRT: goff = 0x3000_0000;
    const PAGES: usize = 16;

    struct Tester {
        virt: usize,
        mgate: MemGate,
    }

    impl profile::Runner for Tester {
        fn pre(&mut self) {
            // create new mapping
            self.virt = VPE::cur()
                .pager()
                .unwrap()
                .map_anon(VIRT, PAGES * cfg::PAGE_SIZE, Perm::RW, MapFlags::PRIVATE)
                .unwrap() as usize;

            // touch all pages to map them
            let buf: *mut u8 = self.virt as *mut u8;
            for p in 0..PAGES {
                let _byte = unsafe { buf.add(p * cfg::PAGE_SIZE).read_volatile() };
            }
        }

        fn run(&mut self) {
            // now access every page via TCU transfer, which triggers a TLB miss in the TCU
            let buf: *mut u8 = self.virt as *mut u8;
            for p in 0..PAGES {
                let page_buf = unsafe { buf.add(p * cfg::PAGE_SIZE) };
                self.mgate
                    .read_bytes(page_buf, 1, (p * cfg::PAGE_SIZE) as goff)
                    .unwrap();
            }
        }

        fn post(&mut self) {
            // remove mapping
            VPE::cur()
                .pager()
                .unwrap()
                .unmap(self.virt as goff)
                .unwrap();
        }
    }

    struct MyResults(profile::Results);

    impl fmt::Display for MyResults {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(
                f,
                "{} cycles (+/- {} with {} runs)",
                self.0.avg() / PAGES as time::Time,
                self.0.stddev() / PAGES as f32,
                self.0.runs()
            )
        }
    }

    let mut prof = profile::Profiler::default().repeats(10).warmup(2);
    let results = MyResults(prof.runner_with_id(
        &mut Tester {
            virt: 0,
            mgate: MemGate::new(PAGES * cfg::PAGE_SIZE, Perm::RW).unwrap(),
        },
        0x90,
    ));

    wv_perf!("TCU read (1 byte) with translate", results);
}
