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
mod logflags;
mod rdwr;
mod serial;

pub use self::logflags::LogFlags;
pub use self::rdwr::{read_object, Read, Write};
pub use self::serial::Serial;

use crate::io::log::{Log, LogColor};
use crate::tcu::TileId;

use core::fmt;

/// Macro for logging (includes a trailing newline)
///
/// The arguments are printed if $flag is enabled (see `$crate::io::loglvl::LogFlags`).
///
/// # Examples
///
/// ```
/// log!(LogFlags::KernEPs, "my log entry: {}, {}", 1, "test");
/// ```
#[macro_export]
macro_rules! log {
    ($flag:expr, $fmt:expr)                   => (
        $crate::log!(@log_impl $flag, concat!($fmt, "\n"))
    );

    ($flag:expr, $fmt:expr, $($arg:tt)*)      => (
        $crate::log!(@log_impl $flag, concat!($fmt, "\n"), $($arg)*)
    );

    (@log_impl $flag:expr, $($args:tt)*)      => ({
        if $crate::util::unlikely($crate::io::should_log($flag)) {
            $crate::io::log_str(format_args!($($args)*));
        }
    });
}

/// Returns whether a log statement with given flag should be executed
///
/// For bench mode, this will only happen if it's Info or Error. Otherwise, it will depend on what
/// logging flags are set in the environment variable LOG.
#[inline(always)]
pub fn should_log(flag: LogFlags) -> bool {
    #[cfg(feature = "bench")]
    let res = flag == LogFlags::Info || flag == LogFlags::Error;
    #[cfg(not(feature = "bench"))]
    let res = log::flags().contains(flag);
    res
}

/// Helper for the log macro to keep the amount of additional code for logging at a minimum
#[cold]
#[inline(never)]
pub fn log_str(fmt: fmt::Arguments<'_>) {
    if let Some(mut l) = Log::get() {
        l.write_fmt(fmt).unwrap();
    }
}

/// Writes the given byte array to the log, showing `addr` as a prefix
///
/// # Safety
///
/// The address range needs to be readable
pub unsafe fn log_bytes(addr: *const u8, len: usize) {
    if let Some(mut l) = log::Log::get() {
        l.dump_bytes(addr, len).unwrap();
    }
}

/// Writes the given slice to the log, showing `addr` as a prefix
pub fn log_slice(slice: &[u8], addr: usize) {
    if let Some(mut l) = log::Log::get() {
        l.dump_slice(slice, addr).unwrap();
    }
}

/// Initializes the I/O module
pub fn init(tile_id: TileId, name: &str) {
    log::init(tile_id, name, LogColor::for_tile(tile_id));
}
