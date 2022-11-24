/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2021 Nils Asmussen, Barkhausen Institut
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

use m3::cell::LazyStaticCell;
use m3::col::Vec;
use m3::env;
use m3::errors::{Code, Error};
use m3::net::IpAddr;
use m3::test::{DefaultWvTester, WvTester};
use m3::{println, wv_run_suite};

mod traw;
mod ttcp;
mod tudp;

pub static NET0_IP: LazyStaticCell<IpAddr> = LazyStaticCell::default();
pub static NET1_IP: LazyStaticCell<IpAddr> = LazyStaticCell::default();
pub static DST_IP: LazyStaticCell<IpAddr> = LazyStaticCell::default();

fn parse_ip(ip: &str) -> IpAddr {
    ip.parse::<IpAddr>()
        .unwrap_or_else(|_| panic!("{}", m3::format!("Invalid IP address: {}", ip)))
}

#[no_mangle]
pub fn main() -> Result<(), Error> {
    let args: Vec<&str> = env::args().collect();
    if args.len() != 4 {
        println!("Usage: {} <net0-IP> <net1-IP> <dst-IP>", args[0]);
        return Err(Error::new(Code::InvArgs));
    }

    NET0_IP.set(parse_ip(args[1]));
    NET1_IP.set(parse_ip(args[2]));
    DST_IP.set(parse_ip(args[3]));

    let mut tester = DefaultWvTester::default();
    wv_run_suite!(tester, traw::run);
    wv_run_suite!(tester, tudp::run);
    wv_run_suite!(tester, ttcp::run);
    println!("{}", tester);
    Ok(())
}
