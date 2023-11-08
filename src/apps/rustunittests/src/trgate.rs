/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

use m3::com::{RGateArgs, RecvGate, SendCap};
use m3::errors::Code;
use m3::test::WvTester;
use m3::{wv_assert_err, wv_run_test};

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, create);
    wv_run_test!(t, destroy);
}

fn create(t: &mut dyn WvTester) {
    wv_assert_err!(t, RecvGate::new(8, 9), Code::InvArgs);
    wv_assert_err!(
        t,
        RecvGate::new_with(RGateArgs::default().sel(1)),
        Code::InvArgs
    );
}

fn destroy(t: &mut dyn WvTester) {
    use m3::cap::Selector;
    use m3::com::{recv_msg, SGateArgs, SendGate};
    use m3::tiles::{Activity, ChildActivity, RunningActivity, Tile};
    use m3::{reply_vmsg, send_recv, wv_assert_eq, wv_assert_ok};

    let tile = wv_assert_ok!(Tile::get("compat|own"));
    let mut child = wv_assert_ok!(ChildActivity::new(tile, "test"));

    let act = {
        let rg = wv_assert_ok!(RecvGate::new_with(
            RGateArgs::default().order(6).msg_order(6)
        ));
        // TODO actually, we could create it in the child, but this is not possible in rust atm
        // because we would need to move rg to the child *and* use it in the parent
        let sg = wv_assert_ok!(SendCap::new_with(SGateArgs::new(&rg).credits(1)));

        wv_assert_ok!(child.delegate_obj(sg.sel()));

        let mut dst = child.data_sink();
        dst.push(sg.sel());

        let act = wv_assert_ok!(child.run(|| {
            let mut t = m3::test::DefaultWvTester::default();
            let sg_sel: Selector = Activity::own().data_source().pop().unwrap();
            let sg = wv_assert_ok!(SendGate::new_bind(sg_sel));

            let mut i = 0;
            for _ in 0..10 {
                wv_assert_ok!(send_recv!(&sg, RecvGate::def(), i, i + 1, i + 2));
                i += 3;
            }
            wv_assert_err!(
                t,
                send_recv!(&sg, RecvGate::def(), i, i + 1, i + 2),
                Code::NoSEP
            );
            Ok(())
        }));

        for i in 0..10 {
            let mut msg = wv_assert_ok!(recv_msg(&rg));
            let (a1, a2, a3): (i32, i32, i32) = (
                wv_assert_ok!(msg.pop()),
                wv_assert_ok!(msg.pop()),
                wv_assert_ok!(msg.pop()),
            );
            wv_assert_eq!(t, a1, i * 3 + 0);
            wv_assert_eq!(t, a2, i * 3 + 1);
            wv_assert_eq!(t, a3, i * 3 + 2);
            wv_assert_ok!(reply_vmsg!(msg, 0));
        }

        act
    };

    wv_assert_eq!(t, act.wait(), Ok(Code::Success));
}
