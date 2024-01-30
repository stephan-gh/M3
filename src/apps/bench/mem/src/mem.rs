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

use m3::com::MemGate;
use m3::env;
use m3::errors::Error;
use m3::kif;
use m3::mem::GlobOff;
use m3::time::{CycleInstant, Profiler};
use m3::{format, vec, wv_perf};

#[no_mangle]
pub fn main() -> Result<(), Error> {
    let tcu = env::args().nth(1).unwrap() == "tcu";

    let buf = vec![0u8; 1024 * 1024];
    let mut buf2 = vec![0u8; 1024 * 1024];

    let mgate = MemGate::new(buf.len() as GlobOff, kif::Perm::W).expect("Unable to create mgate");

    for i in 0..=28 {
        let prof = Profiler::default().repeats(10).warmup(2);
        let size = 1 << i;
        let cur_buf = &buf[0..buf.len().min(size)];

        wv_perf!(
            format!("write {}b with {}b buf", size, size),
            prof.run::<CycleInstant, _>(|| {
                let mut total = 0;
                while total < size {
                    if tcu {
                        mgate.write(cur_buf, 0).expect("Writing failed");
                    }
                    else {
                        buf2[0..cur_buf.len()].copy_from_slice(cur_buf);
                    }
                    total += buf.len();
                }
            })
        );
    }

    Ok(())
}
