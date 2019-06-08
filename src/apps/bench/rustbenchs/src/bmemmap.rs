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

use m3::col::Vec;
use m3::mem::MemMap;
use m3::test;
use m3::profile;

pub fn run(t: &mut dyn test::Tester) {
    run_test!(t, perf_alloc);
    run_test!(t, perf_free);
}

fn perf_alloc() {
    let mut prof = profile::Profiler::new().repeats(10);

    struct MemMapTester {
        map: MemMap,
    }

    impl profile::Runner for MemMapTester {
        fn pre(&mut self) {
            self.map = MemMap::new(0, 0x1000000);
        }
        fn run(&mut self) {
            for _ in 0..100 {
                assert_ok!(self.map.allocate(0x1000, 0x1000));
            }
        }
    }

    let mut tester = MemMapTester {
        map: MemMap::new(0, 0x100000),
    };

    println!("Allocating 100 areas: {}", prof.runner_with_id(&mut tester, 0x10));
}

fn perf_free() {
    let mut prof = profile::Profiler::new().repeats(10);

    struct MemMapTester {
        map: MemMap,
        addrs: Vec<u64>,
        forward: bool,
    }

    impl profile::Runner for MemMapTester {
        fn pre(&mut self) {
            self.map = MemMap::new(0, 0x100000);
            self.addrs.clear();
            for _ in 0..100 {
                self.addrs.push(assert_ok!(self.map.allocate(0x1000, 0x1000)));
            }
        }
        fn run(&mut self) {
            for i in 0..100 {
                let idx = if self.forward { i } else { 100 - i - 1 };
                self.map.free(self.addrs[idx], 0x1000);
            }
            assert_eq!(self.map.size(), (0x100000, 1));
        }
    }

    let mut tester = MemMapTester {
        map: MemMap::new(0, 0x100000),
        addrs: Vec::new(),
        forward: true,
    };
    println!("Freeing 100 areas forward  : {}", prof.runner_with_id(&mut tester, 0x11));

    tester.forward = false;
    println!("Freeing 100 areas backwards: {}", prof.runner_with_id(&mut tester, 0x12));
}
