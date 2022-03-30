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

use crate::boxed::Box;
use crate::cell::{LazyStaticRefCell, RefMut};
use crate::io::Serial;
use crate::tiles::Activity;
use crate::vfs::{BufReader, BufWriter, Fd, GenFileRef};

/// The file descriptor for the standard input stream
pub const STDIN_FILENO: Fd = 0;
/// The file descriptor for the standard output stream
pub const STDOUT_FILENO: Fd = 1;
/// The file descriptor for the standard error stream
pub const STDERR_FILENO: Fd = 2;

static STDIN: LazyStaticRefCell<BufReader<GenFileRef>> = LazyStaticRefCell::default();
static STDOUT: LazyStaticRefCell<BufWriter<GenFileRef>> = LazyStaticRefCell::default();
static STDERR: LazyStaticRefCell<BufWriter<GenFileRef>> = LazyStaticRefCell::default();

/// The standard input stream
pub fn stdin() -> RefMut<'static, BufReader<GenFileRef>> {
    STDIN.borrow_mut()
}
/// The standard output stream
pub fn stdout() -> RefMut<'static, BufWriter<GenFileRef>> {
    STDOUT.borrow_mut()
}
/// The standard error stream
pub fn stderr() -> RefMut<'static, BufWriter<GenFileRef>> {
    STDERR.borrow_mut()
}

pub(crate) fn init() {
    for fd in 0..3 {
        if !Activity::cur().files().exists(fd) {
            Activity::cur().files().set_raw(fd, Box::new(Serial::new()));
        }
    }

    let create_in = |fd| BufReader::new(GenFileRef::new_owned(fd));
    let create_out = |fd| BufWriter::new(GenFileRef::new_owned(fd));

    STDIN.set(create_in(STDIN_FILENO));
    STDOUT.set(create_out(STDOUT_FILENO));
    STDERR.set(create_out(STDERR_FILENO));
}

pub(crate) fn deinit() {
    STDIN.unset();
    STDOUT.unset();
    STDERR.unset();
}
