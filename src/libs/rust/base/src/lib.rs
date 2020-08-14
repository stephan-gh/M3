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

#![feature(alloc_error_handler, allocator_internals)]
#![feature(llvm_asm)]
#![feature(box_into_raw_non_null)]
#![feature(const_fn)]
#![feature(core_intrinsics)]
#![feature(lang_items)]
#![feature(panic_info_message)]
#![default_lib_allocator]
#![no_std]

// for int_enum!
pub extern crate core as _core;
pub extern crate static_assertions;

use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(target_os = "none")] {
        extern crate alloc;

        /// The C library
        pub mod libc {
            pub use crate::arch::libc::*;
        }
    }
    else if #[cfg(target_os = "linux")] {
        #[macro_use]
        extern crate alloc;

        /// The C library
        pub extern crate libc;
    }
}

// Macros
pub use alloc::{format, vec};
pub use static_assertions::const_assert;

// lang stuff
mod lang;
pub use lang::*;

/// Pointer types for heap allocation
pub mod boxed {
    pub use alloc::boxed::Box;
}

/// Thread-safe reference-counting pointers
pub mod sync {
    pub use alloc::sync::{Arc, Weak};
}

#[macro_use]
pub mod io;
#[macro_use]
pub mod util;
#[macro_use]
pub mod test;

pub mod backtrace;
pub mod cell;
pub mod col;
pub mod elf;
pub mod env;
pub mod errors;
pub mod kif;
pub mod math;
pub mod mem;
pub mod pexif;
pub mod profile;
pub mod rc;
pub mod serialize;
pub mod time;

mod arch;

/// An offset in a [`GlobAddr`](mem::GlobAddr)
#[allow(non_camel_case_types)]
pub type goff = u64;

/// Machine-specific functions
#[cfg(target_os = "none")]
pub mod machine {
    pub use crate::arch::machine::*;
}

/// The target-dependent configuration
pub mod cfg {
    pub use crate::arch::cfg::*;
}
/// CPU-specific functions
pub mod cpu {
    pub use crate::arch::cpu::*;
}
/// The Trusted Communication Unit interface
pub mod tcu {
    pub use crate::arch::tcu::*;
}
/// The environment data
pub mod envdata {
    pub use crate::arch::envdata::*;
}
