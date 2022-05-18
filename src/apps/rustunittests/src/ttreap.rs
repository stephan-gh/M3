/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2020 Nils Asmussen, Barkhausen Institut
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

use m3::col::{Treap, Vec};
use m3::test::WvTester;
use m3::{wv_assert_eq, wv_run_test};

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, test_in_order);
    wv_run_test!(t, test_rev_order);
    wv_run_test!(t, test_rand_order);
}

const TEST_NODE_COUNT: u32 = 10;

fn test_in_order(t: &mut dyn WvTester) {
    let vals = (0..TEST_NODE_COUNT).collect::<Vec<u32>>();
    test_add_modify_and_rem(t, &vals);
}

fn test_rev_order(t: &mut dyn WvTester) {
    let vals = (0..TEST_NODE_COUNT).rev().collect::<Vec<u32>>();
    test_add_modify_and_rem(t, &vals);
}

fn test_rand_order(t: &mut dyn WvTester) {
    let vals = [1, 6, 2, 3, 8, 9, 7, 5, 4];
    test_add_modify_and_rem(t, &vals);
}

fn test_add_modify_and_rem(t: &mut dyn WvTester, vals: &[u32]) {
    let mut plus_one = Vec::new();
    for v in vals {
        plus_one.push(v + 1);
    }

    let mut treap = Treap::new();

    // create
    for v in vals {
        treap.insert(*v, v);
    }

    // modify
    for (i, v) in vals.iter().enumerate() {
        treap.set(*v, &plus_one[i]);
    }

    // find all
    for (i, v) in vals.iter().enumerate() {
        let val = treap.get(v);
        wv_assert_eq!(t, val, Some(&&plus_one[i]));
    }

    // remove
    for (i, v) in vals.iter().enumerate() {
        let val = treap.remove(v);
        wv_assert_eq!(t, val, Some(&plus_one[i]));
        wv_assert_eq!(t, treap.get(v), None);
    }
}
