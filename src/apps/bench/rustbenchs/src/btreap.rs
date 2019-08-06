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

use m3::col::Treap;
use m3::profile;
use m3::test;

pub fn run(t: &mut dyn test::WvTester) {
    wv_run_test!(t, insert);
    wv_run_test!(t, find);
    wv_run_test!(t, clear);
}

fn insert() {
    let mut prof = profile::Profiler::default().repeats(100).warmup(50);

    #[derive(Default)]
    struct BTreeTester(Treap<u32, u32>);

    impl profile::Runner for BTreeTester {
        fn pre(&mut self) {
            self.0.clear();
        }

        fn run(&mut self) {
            for i in 0..100 {
                self.0.insert(i, i);
            }
        }
    }

    wv_perf!(
        "Inserting 100 elements",
        prof.runner_with_id(&mut BTreeTester::default(), 0x71)
    );
}

fn find() {
    let mut prof = profile::Profiler::default().repeats(100).warmup(50);

    #[derive(Default)]
    struct BTreeTester(Treap<u32, u32>);

    impl profile::Runner for BTreeTester {
        fn pre(&mut self) {
            for i in 0..100 {
                self.0.insert(i, i);
            }
        }

        fn run(&mut self) {
            for i in 0..100 {
                let val = self.0.get(&i);
                wv_assert_eq!(val, Some(&i));
            }
        }

        fn post(&mut self) {
            self.0.clear();
        }
    }

    wv_perf!(
        "Searching for 100 elements",
        prof.runner_with_id(&mut BTreeTester::default(), 0x72)
    );
}

fn clear() {
    let mut prof = profile::Profiler::default().repeats(100).warmup(50);

    #[derive(Default)]
    struct BTreeTester(Treap<u32, u32>);

    impl profile::Runner for BTreeTester {
        fn pre(&mut self) {
            for i in 0..100 {
                self.0.insert(i, i);
            }
        }

        fn run(&mut self) {
            self.0.clear();
        }
    }

    wv_perf!(
        "Removing 100-element list",
        prof.runner_with_id(&mut BTreeTester::default(), 0x73)
    );
}
