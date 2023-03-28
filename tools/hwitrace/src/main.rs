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

mod error;
mod instrs;
mod trace;

use std::collections::HashMap;
use std::env;

fn usage() -> ! {
    eprintln!(
        "Usage: {} <crossprefix> <bin>...",
        env::args().next().unwrap()
    );
    std::process::exit(1);
}

fn main() -> Result<(), error::Error> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        usage();
    }

    let mut instrs = HashMap::new();
    for f in &args[2..] {
        instrs::parse_instrs(&args[1], &mut instrs, f)?;
    }

    trace::enrich_trace(&instrs)
}
