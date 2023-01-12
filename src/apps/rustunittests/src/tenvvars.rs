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

use m3::col::ToString;
use m3::env;
use m3::errors::Code;
use m3::test::{DefaultWvTester, WvTester};
use m3::tiles::{ActivityArgs, ChildActivity, RunningActivity, Tile};
use m3::{wv_assert_eq, wv_assert_ok, wv_run_test};

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, basics);
    wv_run_test!(t, multi);
    wv_run_test!(t, to_child);
}

fn basics(t: &mut dyn WvTester) {
    wv_assert_eq!(t, env::var("FOO"), None);
    env::set_var("TEST", "value");
    wv_assert_eq!(t, env::var("TEST"), Some("value".to_string()));

    wv_assert_eq!(t, env::vars().len(), 1);
    let vars = env::vars();
    let mut it = vars.iter();
    wv_assert_eq!(
        t,
        it.next(),
        Some(&("TEST".to_string(), "value".to_string()))
    );
    wv_assert_eq!(t, it.next(), None);

    env::remove_var("ABC");
    wv_assert_eq!(t, env::vars().len(), 1);
    env::remove_var("TEST");
    wv_assert_eq!(t, env::vars().len(), 0);
    wv_assert_eq!(t, env::var("TEST"), None);
}

fn multi(t: &mut dyn WvTester) {
    env::set_var("V1", "val1");
    env::set_var("V2", "val2");
    env::set_var("V2", "val3");
    env::set_var("V21", "val=with=eq");
    wv_assert_eq!(t, env::vars().len(), 3);

    let vars = env::vars();
    let mut it = vars.iter();
    wv_assert_eq!(t, it.next(), Some(&("V1".to_string(), "val1".to_string())));
    wv_assert_eq!(t, it.next(), Some(&("V2".to_string(), "val3".to_string())));
    wv_assert_eq!(
        t,
        it.next(),
        Some(&("V21".to_string(), "val=with=eq".to_string()))
    );
    wv_assert_eq!(t, it.next(), None);

    env::remove_var("V2");
    wv_assert_eq!(t, env::vars().len(), 2);
    env::remove_var("V21");
    wv_assert_eq!(t, env::vars().len(), 1);
    env::remove_var("V1");
    wv_assert_eq!(t, env::vars().len(), 0);
}

fn to_child(t: &mut dyn WvTester) {
    env::set_var("V1", "val1");
    env::set_var("V2", "val2");
    env::set_var("V3", "val3");

    let act = wv_assert_ok!(ChildActivity::new_with(
        wv_assert_ok!(Tile::get("compat|own")),
        ActivityArgs::new("child")
    ));

    let run = wv_assert_ok!(act.run(|| {
        let mut t = DefaultWvTester::default();
        wv_assert_eq!(t, env::vars().len(), 3);
        let vars = env::vars();
        let mut it = vars.iter();
        wv_assert_eq!(t, it.next(), Some(&("V1".to_string(), "val1".to_string())));
        wv_assert_eq!(t, it.next(), Some(&("V2".to_string(), "val2".to_string())));
        wv_assert_eq!(t, it.next(), Some(&("V3".to_string(), "val3".to_string())));
        wv_assert_eq!(t, it.next(), None);

        env::remove_var("V2");
        wv_assert_eq!(t, env::vars().len(), 2);
        Ok(())
    }));

    wv_assert_eq!(t, run.wait(), Ok(Code::Success));

    env::remove_var("V3");
    env::remove_var("V2");
    env::remove_var("V1");
    wv_assert_eq!(t, env::vars().len(), 0);
}
