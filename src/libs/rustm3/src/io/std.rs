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

use cell::StaticCell;
use core::mem;
use io::Serial;
use vfs::{BufReader, BufWriter, Fd, FileRef};
use vpe::VPE;

/// The file descriptor for the stanard input stream
pub const STDIN_FILENO: Fd = 0;
/// The file descriptor for the stanard output stream
pub const STDOUT_FILENO: Fd = 1;
/// The file descriptor for the stanard error stream
pub const STDERR_FILENO: Fd = 2;

static STDIN: StaticCell<Option<BufReader<FileRef>>> = StaticCell::new(None);
static STDOUT: StaticCell<Option<BufWriter<FileRef>>> = StaticCell::new(None);
static STDERR: StaticCell<Option<BufWriter<FileRef>>> = StaticCell::new(None);

/// The standard input stream
pub fn stdin() -> &'static mut BufReader<FileRef> {
    STDIN.get_mut().as_mut().unwrap()
}
/// The standard output stream
pub fn stdout() -> &'static mut BufWriter<FileRef> {
    STDOUT.get_mut().as_mut().unwrap()
}
/// The standard error stream
pub fn stderr() -> &'static mut BufWriter<FileRef> {
    STDERR.get_mut().as_mut().unwrap()
}

pub(crate) fn init() {
    for fd in 0..3 {
        if VPE::cur().files().get(fd).is_none() {
            VPE::cur().files().set(fd, Serial::new());
        }
    }

    let create_in = |fd| {
        let f = VPE::cur().files().get(fd).unwrap();
        Some(BufReader::new(FileRef::new(f, fd)))
    };
    let create_out = |fd| {
        let f = VPE::cur().files().get(fd).unwrap();
        Some(BufWriter::new(FileRef::new(f, fd)))
    };

    STDIN.set(create_in(STDIN_FILENO));
    STDOUT.set(create_out(STDOUT_FILENO));
    STDERR.set(create_out(STDERR_FILENO));
}

pub(crate) fn reinit() {
    mem::forget(STDIN.set(None));
    mem::forget(STDOUT.set(None));
    mem::forget(STDERR.set(None));
    init();
}

pub(crate) fn deinit() {
    STDIN.set(None);
    STDOUT.set(None);
    STDERR.set(None);
}
