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

use m3::pes::{Activity, VPEArgs, PE, VPE};
use m3::pexif;
use m3::rc::Rc;
use m3::test;
use m3::{println, wv_assert, wv_assert_ok, wv_run_test};

pub fn run(t: &mut dyn test::WvTester) {
    wv_run_test!(t, calc_pi_local);
    wv_run_test!(t, calc_pi_remote);
}

fn calc_pi_local() {
    if !VPE::cur().pe_desc().has_virtmem() {
        println!("No virtual memory; skipping calc_pi_local test");
        return;
    }

    calc_pi(VPE::cur().pe());
}

fn calc_pi_remote() {
    let pe = wv_assert_ok!(PE::get("clone"));
    calc_pi(&pe);
}

#[allow(clippy::approx_constant)]
const PI_MIN: f64 = 3.141;
#[allow(clippy::approx_constant)]
const PI_MAX: f64 = 3.143;

fn calc_pi(pe: &Rc<PE>) {
    let vpe = wv_assert_ok!(VPE::new_with(pe.clone(), VPEArgs::new("t1")));

    let act = wv_assert_ok!(vpe.run(|| {
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
                wv_assert_ok!(pexif::switch_vpe());
            }

            div += 2.0;
        }

        wv_assert!(pi >= PI_MIN);
        wv_assert!(pi <= PI_MAX);
        println!("PI (Somayaji) on PE{} = {}", VPE::cur().pe_id(), pi);
        0
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
            wv_assert_ok!(pexif::switch_vpe());
        }

        div += 2.0;
    }

    let pi = res * 4.0;
    wv_assert!(pi >= PI_MIN);
    wv_assert!(pi <= PI_MAX);
    println!("PI (Leibniz) on PE{} = {}", VPE::cur().pe_id(), pi);

    wv_assert_ok!(act.wait());
}
