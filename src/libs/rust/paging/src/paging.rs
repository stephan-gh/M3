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

#![feature(asm)]
#![no_std]

extern crate base;

#[cfg(target_arch = "x86_64")]
#[macro_use]
extern crate bitflags;

#[cfg(target_arch = "arm")]
extern crate bitflags;

#[cfg(target_arch = "x86_64")]
#[path = "x86_64/mod.rs"]
mod paging;

#[cfg(target_arch = "arm")]
#[path = "arm/mod.rs"]
mod paging;

pub use paging::*;
