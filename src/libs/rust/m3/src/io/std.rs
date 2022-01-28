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

use crate::cell::{LazyStaticRefCell, RefCell, RefMut};
use crate::io::Serial;
use crate::pes::VPE;
use crate::rc::Rc;
use crate::vfs::{BufReader, BufWriter, Fd, FileRef};

/// The file descriptor for the standard input stream
pub const STDIN_FILENO: Fd = 0;
/// The file descriptor for the standard output stream
pub const STDOUT_FILENO: Fd = 1;
/// The file descriptor for the standard error stream
pub const STDERR_FILENO: Fd = 2;

static STDIN: LazyStaticRefCell<BufReader<FileRef>> = LazyStaticRefCell::default();
static STDOUT: LazyStaticRefCell<BufWriter<FileRef>> = LazyStaticRefCell::default();
static STDERR: LazyStaticRefCell<BufWriter<FileRef>> = LazyStaticRefCell::default();

/// The standard input stream
pub fn stdin() -> RefMut<'static, BufReader<FileRef>> {
    STDIN.borrow_mut()
}
/// The standard output stream
pub fn stdout() -> RefMut<'static, BufWriter<FileRef>> {
    STDOUT.borrow_mut()
}
/// The standard error stream
pub fn stderr() -> RefMut<'static, BufWriter<FileRef>> {
    STDERR.borrow_mut()
}

pub(crate) fn init() {
    for fd in 0..3 {
        if VPE::cur().files().get(fd).is_none() {
            VPE::cur()
                .files()
                .set(fd, Rc::new(RefCell::new(Serial::new())));
        }
    }

    let create_in = |fd| {
        let f = VPE::cur().files().get(fd).unwrap();
        BufReader::new(FileRef::new(f, fd))
    };
    let create_out = |fd| {
        let f = VPE::cur().files().get(fd).unwrap();
        BufWriter::new(FileRef::new(f, fd))
    };

    STDIN.set(create_in(STDIN_FILENO));
    STDOUT.set(create_out(STDOUT_FILENO));
    STDERR.set(create_out(STDERR_FILENO));
}

pub(crate) fn deinit() {
    STDIN.unset();
    STDOUT.unset();
    STDERR.unset();
}
