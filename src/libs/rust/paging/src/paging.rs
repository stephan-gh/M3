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

#[macro_use]
extern crate cfg_if;

cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        #[macro_use]
        extern crate base;
        #[macro_use]
        extern crate bitflags;

        #[path = "x86_64/mod.rs"]
        mod paging;
    }
    else if #[cfg(target_arch = "arm")] {
        extern crate base;
        extern crate bitflags;

        #[path = "arm/mod.rs"]
        mod paging;
    }
}

use base::goff;

pub use paging::*;

/// Logs mapping operations
pub const LOG_MAP: bool = false;
/// Logs detailed mapping operations
pub const LOG_MAP_DETAIL: bool = false;

pub type AllocFrameFunc = extern "C" fn(vpe: u64) -> goff;
pub type XlatePtFunc = extern "C" fn(vpe: u64, phys: goff) -> usize;
