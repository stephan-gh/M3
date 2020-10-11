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

use crate::arch::{cpu, envdata};
use crate::time;

const START_TSC: usize = 0x1FF1_0000;
const STOP_TSC: usize = 0x1FF2_0000;

pub fn start(msg: usize) -> time::Time {
    if envdata::get().platform == envdata::Platform::GEM5.val {
        cpu::gem5_debug(START_TSC | msg)
    }
    else {
        cpu::elapsed_cycles()
    }
}

pub fn stop(msg: usize) -> time::Time {
    if envdata::get().platform == envdata::Platform::GEM5.val {
        cpu::gem5_debug(STOP_TSC | msg)
    }
    else {
        cpu::elapsed_cycles()
    }
}
