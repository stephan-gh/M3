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

use m3::col::DList;
use m3::test;

pub fn run(t: &mut dyn test::WvTester) {
    wv_run_test!(t, create);
    wv_run_test!(t, basics);
    wv_run_test!(t, iter);
    wv_run_test!(t, iter_insert_before);
    wv_run_test!(t, iter_insert_after);
    wv_run_test!(t, iter_remove);
    wv_run_test!(t, objects);
    wv_run_test!(t, push_back);
    wv_run_test!(t, push_front);
}

fn gen_list<T: Clone>(items: &[T]) -> DList<T> {
    let mut l: DList<T> = DList::new();
    for i in items {
        l.push_back((*i).clone());
    }
    l
}

fn create() {
    let l: DList<u32> = DList::new();
    wv_assert_eq!(l.len(), 0);
    wv_assert_eq!(l.iter().next(), None);
}

fn basics() {
    let mut l = gen_list(&[23, 42, 57]);

    wv_assert_eq!(l.front(), Some(&23));
    wv_assert_eq!(l.back(), Some(&57));

    wv_assert_eq!(l.front_mut(), Some(&mut 23));
    wv_assert_eq!(l.back_mut(), Some(&mut 57));
}

#[allow(clippy::option_map_unit_fn)]
fn iter() {
    let mut l = gen_list(&[23, 42, 57]);

    {
        let mut it = l.iter_mut();
        let e = it.next();
        wv_assert_eq!(e, Some(&mut 23));
        wv_assert_eq!(it.peek_prev(), None);
        e.map(|v| *v = 32);

        let e = it.next();
        wv_assert_eq!(e, Some(&mut 42));
        wv_assert_eq!(it.peek_prev(), Some(&mut 32));
        e.map(|v| *v = 24);

        let e = it.next();
        wv_assert_eq!(e, Some(&mut 57));
        wv_assert_eq!(it.peek_prev(), Some(&mut 24));
        e.map(|v| *v = 75);
    }

    wv_assert_eq!(l, gen_list(&[32, 24, 75]));
}

fn iter_insert_before() {
    {
        let mut l = gen_list(&[23, 42, 57]);
        {
            let mut it = l.iter_mut();
            it.insert_before(21);
        }
        wv_assert_eq!(l, gen_list(&[21, 23, 42, 57]));
    }

    {
        let mut l = gen_list(&[23, 42, 57]);
        {
            let mut it = l.iter_mut();
            wv_assert_eq!(it.next(), Some(&mut 23));
            it.insert_before(21);
        }
        wv_assert_eq!(l, gen_list(&[21, 23, 42, 57]));
    }

    {
        let mut l = gen_list(&[23, 42, 57]);
        {
            let mut it = l.iter_mut();
            wv_assert_eq!(it.next(), Some(&mut 23));
            wv_assert_eq!(it.next(), Some(&mut 42));
            it.insert_before(21);
        }
        wv_assert_eq!(l, gen_list(&[23, 21, 42, 57]));
    }

    {
        let mut l = gen_list(&[23, 42, 57]);
        {
            let mut it = l.iter_mut();
            wv_assert_eq!(it.next(), Some(&mut 23));
            wv_assert_eq!(it.next(), Some(&mut 42));
            wv_assert_eq!(it.next(), Some(&mut 57));
            it.insert_before(21);
        }
        wv_assert_eq!(l, gen_list(&[23, 42, 21, 57]));
    }

    {
        let mut l = gen_list(&[23, 42, 57]);
        {
            let mut it = l.iter_mut();
            wv_assert_eq!(it.next(), Some(&mut 23));
            wv_assert_eq!(it.next(), Some(&mut 42));
            wv_assert_eq!(it.next(), Some(&mut 57));
            wv_assert_eq!(it.next(), None);
            it.insert_before(21);
        }
        wv_assert_eq!(l, gen_list(&[23, 42, 21, 57]));
    }

    {
        let mut l = gen_list(&[23, 42, 57]);
        {
            let mut it = l.iter_mut();
            wv_assert_eq!(it.next(), Some(&mut 23));
            it.insert_before(1);
            it.insert_before(2);
            it.insert_before(3);
        }
        wv_assert_eq!(l, gen_list(&[1, 2, 3, 23, 42, 57]));
    }
}

fn iter_insert_after() {
    let mut l = gen_list(&[23, 42, 57]);

    {
        let mut it = l.iter_mut();
        let e = it.next();
        wv_assert_eq!(e, Some(&mut 23));
        it.insert_after(104);
        it.insert_before(44);
        it.insert_before(45);
    }

    wv_assert_eq!(l, gen_list(&[23, 44, 45, 104, 42, 57]));
}

fn iter_remove() {
    {
        let mut l = gen_list(&[23, 42, 57]);

        {
            let mut it = l.iter_mut();
            wv_assert_eq!(it.remove(), None);

            let e = it.next();
            wv_assert_eq!(e, Some(&mut 23));
            wv_assert_eq!(it.remove(), Some(23));

            let e = it.next();
            wv_assert_eq!(e, Some(&mut 42));
            wv_assert_eq!(it.remove(), Some(42));

            let e = it.next();
            wv_assert_eq!(e, Some(&mut 57));
            wv_assert_eq!(it.remove(), Some(57));

            let e = it.next();
            wv_assert_eq!(e, None);
            wv_assert_eq!(it.remove(), None);
        }

        assert!(l.is_empty());
    }

    {
        let mut l = gen_list(&[1, 2, 3]);

        {
            let mut it = l.iter_mut();
            wv_assert_eq!(it.next(), Some(&mut 1));
            wv_assert_eq!(it.next(), Some(&mut 2));
            wv_assert_eq!(it.remove(), Some(2));
            wv_assert_eq!(it.remove(), Some(1));
            wv_assert_eq!(it.remove(), None);
            wv_assert_eq!(it.next(), Some(&mut 3));
        }

        wv_assert_eq!(l, gen_list(&[3]));
    }
}

fn objects() {
    #[derive(Debug, Eq, PartialEq)]
    struct Foo {
        a: u32,
        b: u32,
        c: u32,
    }

    let mut l: DList<Foo> = DList::new();
    l.push_back(Foo { a: 1, b: 2, c: 3 });
    wv_assert_eq!(l.len(), 1);

    {
        let mut it = l.iter();
        wv_assert_eq!(it.next(), Some(&Foo { a: 1, b: 2, c: 3 }));
        wv_assert_eq!(it.next(), None);
    }

    wv_assert_eq!(l.pop_front(), Some(Foo { a: 1, b: 2, c: 3 }));
    wv_assert_eq!(l.pop_front(), None);
}

fn push_back() {
    let mut l = DList::new();

    l.push_back(1);
    l.push_back(2);
    l.push_back(3);

    wv_assert_eq!(l, gen_list(&[1, 2, 3]));
}

fn push_front() {
    let mut l = DList::new();

    l.push_front(1);
    l.push_front(2);
    l.push_front(3);

    wv_assert_eq!(l, gen_list(&[3, 2, 1]));
}
