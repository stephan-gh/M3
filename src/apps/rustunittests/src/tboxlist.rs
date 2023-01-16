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

use core::fmt;

use m3::boxed::Box;
use m3::col::{BoxList, BoxRef};
use m3::test::WvTester;
use m3::{impl_boxitem, wv_assert_eq, wv_assert_some, wv_run_test};

struct TestItem {
    data: u32,
    prev: Option<BoxRef<TestItem>>,
    next: Option<BoxRef<TestItem>>,
}

impl_boxitem!(TestItem);

impl PartialEq for TestItem {
    fn eq(&self, other: &TestItem) -> bool {
        self.data == other.data
    }
}

impl fmt::Debug for TestItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "data={}", self.data)
    }
}

impl TestItem {
    pub fn new(data: u32) -> Self {
        TestItem {
            data,
            prev: None,
            next: None,
        }
    }
}

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, create);
    wv_run_test!(t, basics);
    wv_run_test!(t, iter);
    wv_run_test!(t, iter_remove);
    wv_run_test!(t, push_back);
    wv_run_test!(t, push_front);
}

fn gen_list(items: &[u32]) -> BoxList<TestItem> {
    let mut l: BoxList<TestItem> = BoxList::new();
    for i in items {
        l.push_back(Box::new(TestItem::new(*i)));
    }
    l
}

fn create(t: &mut dyn WvTester) {
    let l: BoxList<TestItem> = BoxList::new();
    wv_assert_eq!(t, l.len(), 0);
    wv_assert_eq!(t, l.iter().next(), None);
}

fn basics(t: &mut dyn WvTester) {
    let mut l = gen_list(&[23, 42, 57]);

    wv_assert_eq!(t, l.len(), 3);
    wv_assert_eq!(t, l.front().unwrap().data, 23);
    wv_assert_eq!(t, l.back().unwrap().data, 57);

    wv_assert_eq!(t, l.front_mut().unwrap().data, 23);
    wv_assert_eq!(t, l.back_mut().unwrap().data, 57);
}

fn iter(t: &mut dyn WvTester) {
    let mut l: BoxList<TestItem> = gen_list(&[23, 42, 57]);

    {
        let mut it = l.iter_mut();
        let e = wv_assert_some!(it.next());
        wv_assert_eq!(t, e.data, 23);
        e.data = 32;

        let e = wv_assert_some!(it.next());
        wv_assert_eq!(t, e.data, 42);
        e.data = 24;

        let e = wv_assert_some!(it.next());
        wv_assert_eq!(t, e.data, 57);
        e.data = 75;
    }

    wv_assert_eq!(t, l, gen_list(&[32, 24, 75]));
}

fn iter_remove(t: &mut dyn WvTester) {
    {
        let mut l = gen_list(&[23, 42, 57]);

        {
            let mut it = l.iter_mut();
            wv_assert_eq!(t, it.remove(), None);

            let e = it.next();
            wv_assert_eq!(t, e.as_ref().unwrap().data, 23);
            wv_assert_eq!(t, it.remove().unwrap().data, 23);

            let e = it.next();
            wv_assert_eq!(t, e.as_ref().unwrap().data, 42);
            wv_assert_eq!(t, it.remove().unwrap().data, 42);

            let e = it.next();
            wv_assert_eq!(t, e.as_ref().unwrap().data, 57);
            wv_assert_eq!(t, it.remove().unwrap().data, 57);

            let e = it.next();
            wv_assert_eq!(t, e, None);
            wv_assert_eq!(t, it.remove(), None);
        }

        assert!(l.is_empty());
    }

    {
        let mut l = gen_list(&[1, 2, 3]);

        {
            let mut it = l.iter_mut();
            wv_assert_eq!(t, it.next().as_ref().unwrap().data, 1);
            wv_assert_eq!(t, it.next().as_ref().unwrap().data, 2);
            wv_assert_eq!(t, it.remove().unwrap().data, 2);
            wv_assert_eq!(t, it.remove().unwrap().data, 1);
            wv_assert_eq!(t, it.remove(), None);
            wv_assert_eq!(t, it.next().as_ref().unwrap().data, 3);
        }

        wv_assert_eq!(t, l, gen_list(&[3]));
    }
}

fn push_back(t: &mut dyn WvTester) {
    let mut l = BoxList::new();

    l.push_back(Box::new(TestItem::new(1)));
    l.push_back(Box::new(TestItem::new(2)));
    l.push_back(Box::new(TestItem::new(3)));

    wv_assert_eq!(t, l, gen_list(&[1, 2, 3]));
}

fn push_front(t: &mut dyn WvTester) {
    let mut l = BoxList::new();

    l.push_front(Box::new(TestItem::new(1)));
    l.push_front(Box::new(TestItem::new(2)));
    l.push_front(Box::new(TestItem::new(3)));

    wv_assert_eq!(t, l, gen_list(&[3, 2, 1]));
}
