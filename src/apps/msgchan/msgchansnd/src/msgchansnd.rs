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

use m3::cell::StaticCell;
use m3::com::{recv_msg, RecvGate, SendGate};
use m3::env;
use m3::{println, reply_vmsg, send_recv, wv_assert_eq, wv_assert_ok};

static FAILED: StaticCell<u32> = StaticCell::new(0);

extern "C" fn wvtest_failed() {
    FAILED.set(FAILED.get() + 1);
}

#[no_mangle]
pub fn main() -> i32 {
    let sgate = wv_assert_ok!(SendGate::new_named("chan"));
    let mut rgate = wv_assert_ok!(RecvGate::new_named(env::args().nth(1).unwrap()));
    wv_assert_ok!(rgate.activate());

    let mut val = 42;
    for _ in 0..16 {
        println!("Sending {}", val);
        wv_assert_ok!(send_recv!(&sgate, RecvGate::def(), val));

        let mut reply = wv_assert_ok!(recv_msg(&rgate));
        let res = reply.pop::<u64>();
        println!("Received {:?}", res);
        wv_assert_eq!(res, Ok(val + 1));
        wv_assert_ok!(reply_vmsg!(reply, 0));

        val += 100;
    }

    if FAILED.get() > 0 {
        println!("\x1B[1;31m{} tests failed\x1B[0;m", FAILED.get());
    }
    else {
        println!("\x1B[1;32mAll tests successful!\x1B[0;m");
    }

    0
}
