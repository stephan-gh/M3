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
use crate::vfs::{BufReader, BufWriter, Fd, File, FileRef};

/// The file descriptor for the standard input stream
pub const STDIN_FILENO: Fd = 0;
/// The file descriptor for the standard output stream
pub const STDOUT_FILENO: Fd = 1;
/// The file descriptor for the standard error stream
pub const STDERR_FILENO: Fd = 2;

static STDIN: LazyStaticRefCell<BufReader<FileRef<dyn File>>> = LazyStaticRefCell::default();
static STDOUT: LazyStaticRefCell<BufWriter<FileRef<dyn File>>> = LazyStaticRefCell::default();
static STDERR: LazyStaticRefCell<BufWriter<FileRef<dyn File>>> = LazyStaticRefCell::default();

/// The standard input stream
pub fn stdin() -> RefMut<'static, BufReader<FileRef<dyn File>>> {
    STDIN.borrow_mut()
}
/// The standard output stream
pub fn stdout() -> RefMut<'static, BufWriter<FileRef<dyn File>>> {
    STDOUT.borrow_mut()
}
/// The standard error stream
pub fn stderr() -> RefMut<'static, BufWriter<FileRef<dyn File>>> {
    STDERR.borrow_mut()
}

pub(crate) fn init() {
    for fd in 0..3 {
        if !Activity::own().files().exists(fd) {
            Activity::own().files().set_raw(fd, Box::new(Serial::new()));
        }
    }

    STDIN.set(BufReader::new(FileRef::new_owned(STDIN_FILENO)));
    STDOUT.set(BufWriter::new(FileRef::new_owned(STDOUT_FILENO)));
    STDERR.set(BufWriter::new(FileRef::new_owned(STDERR_FILENO)));
}

pub(crate) fn deinit() {
    STDIN.unset();
    STDOUT.unset();
    STDERR.unset();
}
