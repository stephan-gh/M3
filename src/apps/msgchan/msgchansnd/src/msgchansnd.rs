/*
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
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

#![no_std]

use m3::com::{recv_msg, RecvGate, SendGate};
use m3::env;
use m3::errors::Error;
use m3::test::{DefaultWvTester, WvTester};
use m3::{println, reply_vmsg, send_recv, wv_assert_eq, wv_assert_ok};

#[no_mangle]
pub fn main() -> Result<(), Error> {
    let mut tester = DefaultWvTester::default();

    let sgate = wv_assert_ok!(SendGate::new_named("chan"));
    let rgate = wv_assert_ok!(RecvGate::new_named(env::args().nth(1).unwrap()));

    let mut val = 42;
    for _ in 0..16 {
        println!("Sending {}", val);
        wv_assert_ok!(send_recv!(&sgate, RecvGate::def(), val));

        let mut reply = wv_assert_ok!(recv_msg(&rgate));
        let res = reply.pop::<u64>();
        println!("Received {:?}", res);
        wv_assert_eq!(tester, res, Ok(val + 1));
        wv_assert_ok!(reply_vmsg!(reply, 0));

        val += 100;
    }

    println!("{}", tester);

    Ok(())
}
