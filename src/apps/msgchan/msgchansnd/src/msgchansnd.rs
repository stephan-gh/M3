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

use m3::com::{RecvGate, SendGate};
use m3::send_recv;

#[no_mangle]
pub fn main() -> i32 {
    let sgate = SendGate::new_named("chan").expect("Unable to create SendGate");

    let mut val = 73;
    for _ in 0..16 {
        send_recv!(&sgate, RecvGate::def(), val).expect("send failed");
        val += 8127312;
    }

    0
}
