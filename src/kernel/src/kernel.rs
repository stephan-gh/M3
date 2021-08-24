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

#![feature(core_intrinsics)]
#![feature(ptr_internals)]
#![no_std]

#[cfg(not(target_vendor = "host"))]
extern crate paging;

#[macro_use]
mod log;

pub mod arch;
mod args;
mod cap;
mod com;
mod ktcu;
mod mem;
mod pes;
mod platform;
mod slab;
mod syscalls;
mod workloop;
