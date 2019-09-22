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
#![feature(const_fn)]
#![feature(core_intrinsics)]
#![feature(trace_macros)]
#![no_std]

#[macro_use]
extern crate base;
#[macro_use]
extern crate bitflags;

// init stuff
#[cfg(target_os = "none")]
pub use arch::init::{env_run, exit};
#[cfg(target_os = "linux")]
pub use arch::init::{exit, rust_deinit, rust_init};

#[macro_use]
pub mod io;
#[macro_use]
pub mod com;
mod dtuif;

pub mod dtu {
    pub use base::dtu::*;
    pub use dtuif::*;
}

pub use base::{
    backtrace,
    boxed,
    cell,
    cfg,
    col,
    cpu,

    elf,
    env,
    errors,
    format,
    function,
    // types
    goff,
    impl_boxitem,
    int_enum,
    kif,
    // modules
    libc,
    log,
    mem,
    profile,
    rc,
    serialize,
    sync,
    test,
    time,
    util,
    // macros
    vec,
    wv_assert_eq,
    wv_assert_err,
    wv_assert_ok,
    wv_assert_some,

    wv_perf,
    wv_run_suite,
    wv_run_test,
};

pub mod cap;
pub mod server;
pub mod session;
pub mod syscalls;
pub mod vfs;
pub mod vpe;

mod arch;
