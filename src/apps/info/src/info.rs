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

use core::fmt::Display;

use m3::pes::VPE;
use m3::println;

struct MemQuota {
    total: usize,
    avail: usize,
}

impl Display for MemQuota {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let total = self.total / 1024;
        write!(f, "{:7}K/{:7}K", self.avail / 1024, total,)
    }
}

struct EPQuota {
    total: u32,
    avail: u32,
}

impl Display for EPQuota {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:3}/{:3}", self.avail, self.total,)
    }
}

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
        if let Ok(vpe) = VPE::cur().resmng().unwrap().get_vpe_info(i) {
            let umem = MemQuota {
                total: vpe.total_umem,
                avail: vpe.avail_umem,
            };
            let kmem = MemQuota {
                total: vpe.total_kmem,
                avail: vpe.avail_kmem,
            };
            let eps = EPQuota {
                total: vpe.total_eps,
                avail: vpe.avail_eps,
            };
            println!(
                "{:2} {:2} {} {:2}:{} {} {:0l$}{}",
                vpe.id,
                vpe.pe,
                eps,
                vpe.mem_pool,
                umem,
                kmem,
                "",
                vpe.name,
                l = vpe.layer as usize * 2,
            );
        }
        else {
            println!("Unable to get info about VPE with idx {}", i);
        }
    }
    0
}
