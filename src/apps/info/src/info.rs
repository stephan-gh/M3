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

use m3::goff;
use m3::pes::VPE;
use m3::println;

fn count_digits(mut i: goff) -> usize {
    let mut digits = 1;
    while i >= 10 {
        i /= 10;
        digits += 1;
    }
    digits
}

#[no_mangle]
pub fn main() -> i32 {
    let (num, _) = VPE::cur()
        .resmng()
        .unwrap()
        .get_vpe_count()
        .expect("Unable to get VPE count");
    println!("{:2} {:2} {:15} {}", "ID", "PE", "Free/Avail Mem", "Name");
    for i in 0..num {
        if let Ok(vpe) = VPE::cur().resmng().unwrap().get_vpe_info(i) {
            let avail_mem = vpe.avail_mem / (1024 * 1024);
            let total_mem = vpe.total_mem / (1024 * 1024);
            println!(
                "{:2} {:2} {:-6}M/{}M{:0m$} {:0l$}{}",
                vpe.id,
                vpe.pe,
                avail_mem,
                total_mem,
                "",
                "",
                vpe.name,
                m = 6 - count_digits(total_mem),
                l = vpe.layer as usize * 2,
            );
        }
        else {
            println!("Unable to get info about VPE with idx {}", i);
        }
    }
    0
}
