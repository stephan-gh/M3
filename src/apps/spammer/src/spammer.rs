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

#![no_std]

use m3::env;
use m3::format;
use m3::io::{stdout, Write};
use m3::time::{TimeDuration, TimeInstant};

const FREQ: TimeDuration = TimeDuration::from_millis(1);

#[no_mangle]
pub fn main() -> i32 {
    let c = env::args()
        .nth(1)
        .expect(&format!("Usage: {} <str>", env::args().next().unwrap()));

    loop {
        let now = TimeInstant::now();
        m3::print!("{}", c);
        stdout().flush().unwrap();

        while TimeInstant::now() < now + FREQ {}
    }
}
