/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

#![allow(internal_features)]
#![feature(allocator_internals)]
#![feature(core_intrinsics)]
#![feature(maybe_uninit_write_slice)]
#![default_lib_allocator]
#![cfg_attr(not(feature = "linux"), no_std)]

extern crate alloc;
pub extern crate core as _core;

// Macros
pub use alloc::{format, vec};
pub use static_assertions::const_assert;

mod arch;

/// Pointer types for heap allocation
pub mod boxed {
    pub use alloc::boxed::Box;
}

/// CPU-specific functions
pub mod cpu {
    pub use crate::arch::{CPUOps, CPU};
}

/// Thread-safe reference-counting pointers
#[cfg(target_has_atomic = "ptr")]
pub mod sync {
    pub use alloc::sync::{Arc, Weak};
}

/// Types to work with borrowed data
pub mod borrow {
    pub use alloc::borrow::{Borrow, BorrowMut, Cow, ToOwned};
}

#[macro_use]
pub mod io;
#[macro_use]
pub mod util;

pub mod backtrace;
pub mod cell;
pub mod cfg;
pub mod col;
pub mod crypto;
pub mod elf;
pub mod env;
pub mod errors;
pub mod kif;
pub mod libc;
pub mod machine;
pub mod mem;
pub mod msgqueue;
pub mod quota;
pub mod rc;
pub mod serialize;
pub mod tcu;
pub mod time;
pub mod tmif;

#[cfg(feature = "coverage")]
pub use minicov;

pub use serde;

#[cfg(feature = "linux")]
pub use arch::linux;
