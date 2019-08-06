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

//! Input/output abstractions.

mod serial;
mod std;

pub use self::serial::*;
pub use self::std::{stderr, stdin, stdout};
pub use self::std::{STDERR_FILENO, STDIN_FILENO, STDOUT_FILENO};
pub use base::io::{read_object, Read, Serial, Write};

/// Uses stdout to print `$fmt` with given arguments.
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ({
        #[allow(unused_imports)]
        use $crate::io::Write;
        $crate::io::stdout().write_fmt(format_args!($($arg)*)).unwrap();
    });
}

/// Uses stdout to print `$fmt` with given arguments and a newline.
#[macro_export]
macro_rules! println {
    ()                       => (print!("\n"));
    ($fmt:expr)              => (print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (print!(concat!($fmt, "\n"), $($arg)*));
}

pub(crate) fn init() {
    ::base::io::init();
    std::init();
}

pub(crate) fn reinit() {
    ::base::io::reinit();
    std::reinit();
}

pub(crate) fn deinit() {
    std::deinit();
}
