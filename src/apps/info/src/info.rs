/*
 * Copyright (C) 2021-2022 Nils Asmussen, Barkhausen Institut
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

use m3::errors::Error;
use m3::println;
use m3::tiles::Activity;

#[no_mangle]
pub fn main() -> Result<(), Error> {
    let (num, _) = Activity::own()
        .resmng()
        .unwrap()
        .get_activity_count()
        .expect("Unable to get Activity count");
    println!(
        "{:2} | {:5} | {:>10} | {:>22} | {:>14} | {:>14} | {:>12} | Name",
        "ID", "Tile", "Endpoints", "Time", "UserMem", "KernelMem", "Pagetables"
    );
    for i in 0..num {
        match Activity::own().resmng().unwrap().get_activity_info(i) {
            Ok(act) => {
                println!(
                    "{:2} | {:5} | {:2}:{:3}/{:3} | {:4}:{:6}us/{:6}us | {:2}:{:4}M/{:4}M | {:2}:{:4}M/{:4}M | {:4}:{:3}/{:3} | {:0l$}{}",
                    act.id,
                    act.tile,
                    act.eps.id(),
                    act.eps.remaining(),
                    act.eps.total(),
                    act.time.id(),
                    act.time.remaining().as_micros(),
                    act.time.total().as_micros(),
                    act.umem.id(),
                    act.umem.remaining() / (1024 * 1024),
                    act.umem.total() / (1024 * 1024),
                    act.kmem.id(),
                    act.kmem.remaining() / (1024 * 1024),
                    act.kmem.total() / (1024 * 1024),
                    act.pts.id(),
                    act.pts.remaining(),
                    act.pts.total(),
                    "",
                    act.name,
                    l = act.layer as usize * 2,
                );
            },
            Err(e) => println!(
                "Unable to get info about Activity with idx {}: {:?}",
                i,
                e.code()
            ),
        }
    }

    Ok(())
}
