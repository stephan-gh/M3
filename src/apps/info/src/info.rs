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

use m3::pes::VPE;
use m3::println;

#[no_mangle]
pub fn main() -> i32 {
    let (num, _) = VPE::cur()
        .resmng()
        .unwrap()
        .get_vpe_count()
        .expect("Unable to get VPE count");
    println!(
        "{:2} {:2} {:>7} {:>20} {:>17} {}",
        "ID", "PE", "EPs", "UserMem", "KernelMem", "Name"
    );
    for i in 0..num {
        match VPE::cur().resmng().unwrap().get_vpe_info(i) {
            Ok(vpe) => {
                println!(
                    "{:2} {:2} {:3}/{:3} {:2}:{:7}K/{:7}K {:7}K/{:7}K {:0l$}{}",
                    vpe.id,
                    vpe.pe,
                    vpe.avail_eps,
                    vpe.total_eps,
                    vpe.mem_pool,
                    vpe.avail_umem / 1024,
                    vpe.total_umem / 1024,
                    vpe.avail_kmem / 1024,
                    vpe.total_kmem / 1024,
                    "",
                    vpe.name,
                    l = vpe.layer as usize * 2,
                );
            },
            Err(e) => println!(
                "Unable to get info about VPE with idx {}: {:?}",
                i,
                e.code()
            ),
        }
    }
    0
}
