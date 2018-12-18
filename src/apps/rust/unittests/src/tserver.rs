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

use m3::session::ClientSession;
use m3::col::String;
use m3::errors::Code;
use m3::test;
use m3::com::{RecvGate, SendGate};

pub fn run(t: &mut test::Tester) {
    run_test!(t, testmsgs);
}

pub fn testmsgs() {
    {
        let sess = assert_ok!(ClientSession::new("testmsgs", 0));
        let sel = assert_ok!(sess.obtain_obj());
        let mut sgate = SendGate::new_bind(sel);

        for _ in 0..5 {
            let mut reply = assert_ok!(send_recv!(&mut sgate, RecvGate::def(), 0, "123456"));
            let resp: String = reply.pop();
            assert_eq!(resp, "654321");
        }
    }

    {
        let sess = assert_ok!(ClientSession::new("testmsgs", 0));
        let sel = assert_ok!(sess.obtain_obj());
        let mut sgate = SendGate::new_bind(sel);

        let mut reply = assert_ok!(send_recv!(&mut sgate, RecvGate::def(), 0, "123456"));
        let resp: String = reply.pop();
        assert_eq!(resp, "654321");

        assert_err!(send_recv!(&mut sgate, RecvGate::def(), 0, "123456"), Code::InvEP);
    }
}
