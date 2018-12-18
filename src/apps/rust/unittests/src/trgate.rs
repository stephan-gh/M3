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

use m3::com::{recv_msg, RecvGate, RGateArgs, SendGate, SGateArgs};
use m3::boxed::Box;
use m3::errors::Code;
use m3::test;
use m3::vpe::{Activity, VPE};

pub fn run(t: &mut test::Tester) {
    run_test!(t, create);
    run_test!(t, destroy);
}

fn create() {
    assert_err!(RecvGate::new(8, 9), Code::InvArgs);
    assert_err!(RecvGate::new_with(RGateArgs::new().sel(1)), Code::InvArgs);
}

fn destroy() {
    let mut child = assert_ok!(VPE::new("test"));

    let act = {
        let mut rg = assert_ok!(RecvGate::new_with(RGateArgs::new().order(6).msg_order(6)));
        // TODO actually, we could create it in the child, but this is not possible in rust atm
        // because we would need to move rg to the child *and* use it in the parent
        let sg = assert_ok!(SendGate::new_with(SGateArgs::new(&rg).credits(64)));

        assert_ok!(child.delegate_obj(sg.sel()));

        let act = assert_ok!(child.run(Box::new(move || {
            let mut i = 0;
            for _ in 0..10 {
                assert_ok!(send_recv!(&sg, RecvGate::def(), i, i + 1, i + 2));
                i += 3;
            }
            assert_err!(send_recv!(&sg, RecvGate::def(), i, i + 1, i + 2), Code::InvEP);
            0
        })));

        assert_ok!(rg.activate());

        for i in 0..10 {
            let mut msg = assert_ok!(recv_msg(&rg));
            let (a1, a2, a3): (i32, i32, i32) = (msg.pop(), msg.pop(), msg.pop());
            assert_eq!(a1, i * 3 + 0);
            assert_eq!(a2, i * 3 + 1);
            assert_eq!(a3, i * 3 + 2);
            assert_ok!(reply_vmsg!(msg, 0));
        }

        act
    };

    assert_eq!(act.wait(), Ok(0));
}
