/*
 * Copyright (C) 2020-2022 Nils Asmussen, Barkhausen Institut
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

use m3::rc::Rc;
use m3::test::{DefaultWvTester, WvTester};
use m3::tiles::{Activity, ActivityArgs, ChildActivity, RunningActivity, Tile};
use m3::tmif;
use m3::{println, wv_assert, wv_assert_ok, wv_run_test};

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, calc_pi_local);
    wv_run_test!(t, calc_pi_remote);
}

fn calc_pi_local(t: &mut dyn WvTester) {
    if !Activity::own().tile_desc().has_virtmem() {
        println!("No virtual memory; skipping calc_pi_local test");
        return;
    }

    calc_pi(t, Activity::own().tile());
}

fn calc_pi_remote(t: &mut dyn WvTester) {
    let tile = wv_assert_ok!(Tile::get("compat"));
    calc_pi(t, &tile);
}

#[allow(clippy::approx_constant)]
const PI_MIN: f64 = 3.141;
#[allow(clippy::approx_constant)]
const PI_MAX: f64 = 3.143;

fn calc_pi(t: &mut dyn WvTester, tile: &Rc<Tile>) {
    let act = wv_assert_ok!(ChildActivity::new_with(
        tile.clone(),
        ActivityArgs::new("t1")
    ));

    let act = wv_assert_ok!(act.run(|| {
        let mut t = DefaultWvTester::default();
        let steps = 1000;
        let mut pi = 3.0;
        let mut div = 3.0;
        for i in 0..steps {
            let denom = (div * div * div) - div;
            if i % 2 == 0 {
                pi += 4.0 / denom;
            }
            else {
                pi -= 4.0 / denom;
            }

            // yield every now and then to test if the FPU registers are saved/restored correctly
            if i % 10 == 0 {
                wv_assert_ok!(tmif::switch_activity());
            }

            div += 2.0;
        }

        wv_assert!(t, pi >= PI_MIN);
        wv_assert!(t, pi <= PI_MAX);
        println!("PI (Somayaji) on {} = {}", Activity::own().tile_id(), pi);
        Ok(())
    }));

    let steps = 1000;
    let mut res = 1.0;
    let mut div = 3.0;
    for i in 0..steps {
        if i % 2 == 0 {
            res -= 1.0 / div;
        }
        else {
            res += 1.0 / div;
        }

        if i % 10 == 0 {
            wv_assert_ok!(tmif::switch_activity());
        }

        div += 2.0;
    }

    let pi = res * 4.0;
    wv_assert!(t, pi >= PI_MIN);
    wv_assert!(t, pi <= PI_MAX);
    println!("PI (Leibniz) on {} = {}", Activity::own().tile_id(), pi);

    wv_assert_ok!(act.wait());
}
