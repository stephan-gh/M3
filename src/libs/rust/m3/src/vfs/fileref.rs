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

use core::fmt;
use core::ops::Deref;
use errors::Error;
use goff;
use io::{Read, Write};
use kif;
use pes::VPE;
use session::{MapFlags, Pager};
use vfs::filetable::Fd;
use vfs::{FileHandle, Map, Seek, SeekMode};

/// A reference to an open file that is closed on drop.
#[derive(Clone)]
pub struct FileRef {
    file: FileHandle,
    fd: Fd,
}

impl FileRef {
    /// Creates new file reference for given file and file descriptor.
    pub fn new(file: FileHandle, fd: Fd) -> Self {
        FileRef { file, fd }
    }

    /// Returns the file descriptor.
    pub fn fd(&self) -> Fd {
        self.fd
    }

    /// Returns the file.
    pub fn handle(&self) -> FileHandle {
        self.file.clone()
    }
}

impl Drop for FileRef {
    fn drop(&mut self) {
        VPE::cur().files().remove(self.fd);
    }
}

impl Deref for FileRef {
    type Target = FileHandle;

    fn deref(&self) -> &FileHandle {
        &self.file
    }
}

impl Read for FileRef {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        self.file.borrow_mut().read(buf)
    }
}

impl Write for FileRef {
    fn flush(&mut self) -> Result<(), Error> {
        self.file.borrow_mut().flush()
    }

    fn sync(&mut self) -> Result<(), Error> {
        self.file.borrow_mut().sync()
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        self.file.borrow_mut().write(buf)
    }
}

impl Seek for FileRef {
    fn seek(&mut self, off: usize, whence: SeekMode) -> Result<usize, Error> {
        self.file.borrow_mut().seek(off, whence)
    }
}

impl Map for FileRef {
    fn map(
        &self,
        pager: &Pager,
        virt: goff,
        off: usize,
        len: usize,
        prot: kif::Perm,
        flags: MapFlags,
    ) -> Result<(), Error> {
        self.file.borrow().map(pager, virt, off, len, prot, flags)
    }
}

impl fmt::Debug for FileRef {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "FileRef[fd={}, file={:?}]", self.fd, self.file.borrow())
    }
}
