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

//! Input/output abstractions

mod serial;
mod std;

pub use self::std::{stderr, stdin, stdout};
pub use self::std::{STDERR_FILENO, STDIN_FILENO, STDOUT_FILENO};
pub use base::io::{log_bytes, log_slice, read_object, LogFlags, Read, Serial, Write};

/// Uses stdout to print `$fmt` with given arguments
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ({
        use $crate::io::Write;
        $crate::io::stdout().write_fmt(format_args!($($arg)*)).unwrap();
    });
}

/// Uses stdout to print `$fmt` with given arguments and a newline
#[macro_export]
macro_rules! println {
    ()                       => ($crate::print!("\n"));
    ($fmt:expr)              => ($crate::print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::print!(concat!($fmt, "\n"), $($arg)*));
}

pub(crate) fn init() {
    base::io::init(
        crate::env::get().tile_id(),
        crate::env::args().next().unwrap_or("Unknown"),
    );
    std::init();
}

pub(crate) fn deinit() {
    std::deinit();
}
