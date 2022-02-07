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

//! Contains the modules for serial output, logging, etc.

pub mod log;
mod rdwr;
mod serial;

pub use self::rdwr::{read_object, Read, Write};
pub use self::serial::Serial;

use crate::arch;
use crate::util;

/// Macro for logging (includes a trailing newline)
///
/// The arguments are printed if `$type` is enabled.
///
/// # Examples
///
/// ```
/// log!(SYSC, "my log entry: {}, {}", 1, "test");
/// ```
#[macro_export]
macro_rules! log {
    ($type:expr, $fmt:expr)                   => (
        $crate::llog!(@log_impl $type, concat!($fmt, "\n"))
    );

    ($type:expr, $fmt:expr, $($arg:tt)*)      => (
        $crate::llog!(@log_impl $type, concat!($fmt, "\n"), $($arg)*)
    );
}

/// Macro for library-internal logging (includes a trailing newline)
///
/// The arguments are printed if `$crate::io::log::$type` is enabled.
#[macro_export]
macro_rules! llog {
    ($type:tt, $fmt:expr)                   => (
        llog!(@log_impl $crate::io::log::$type, concat!($fmt, "\n"))
    );

    ($type:tt, $fmt:expr, $($arg:tt)*)      => (
        llog!(@log_impl $crate::io::log::$type, concat!($fmt, "\n"), $($arg)*)
    );

    (@log_impl $type:expr, $($args:tt)*)    => ({
        if $type {
            #[allow(unused_imports)]
            use $crate::io::Write;
            if let Some(mut l) = $crate::io::log::Log::get() {
                l.write_fmt(format_args!($($args)*)).unwrap();
            }
        }
    });
}

/// Writes the given byte array to the log, showing `addr` as a prefix.
///
/// # Safety
///
/// The address range needs to be readable
pub unsafe fn log_bytes(addr: *const u8, len: usize) {
    if let Some(mut l) = log::Log::get() {
        l.dump_bytes(addr, len).unwrap();
    }
}

/// Writes the given slice to the log, showing `addr` as a prefix.
pub fn log_slice(slice: &[u8], addr: usize) {
    if let Some(mut l) = log::Log::get() {
        l.dump_slice(slice, addr).unwrap();
    }
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn init_rust_io(tile_id: u32, name: *const i8) {
    init(tile_id as u64, util::cstr_to_str(name));
}

/// Initializes the I/O module
pub fn init(tile_id: u64, name: &str) {
    arch::serial::init();
    log::init(tile_id, name);
}
