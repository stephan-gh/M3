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

use core::any::Any;
use core::fmt;
use core::marker::PhantomData;
use core::ops::Deref;
use core::ops::DerefMut;

use crate::cap::Selector;
use crate::col::Vec;
use crate::errors::Error;
use crate::goff;
use crate::io::{Read, Write};
use crate::kif;
use crate::session::{HashInput, HashOutput, HashSession, MapFlags, Pager};
use crate::tiles::{Activity, StateSerializer};
use crate::vfs::{Fd, File, FileEvent, Map, Seek, SeekMode};

pub type GenFileRef = FileRef<dyn File>;

#[derive(Clone)]
pub struct FileRef<T: ?Sized> {
    fd: Fd,
    close: bool,
    phantom: PhantomData<T>,
}

impl<T: ?Sized> FileRef<T> {
    /// Creates new file reference for the given file descriptor. The file is not closed on drop.
    pub fn new(fd: Fd) -> Self {
        FileRef {
            fd,
            close: false,
            phantom: PhantomData::default(),
        }
    }

    /// Creates new file reference for the given file descriptor. On drop, the file is closed.
    pub fn new_owned(fd: Fd) -> Self {
        FileRef {
            fd,
            close: true,
            phantom: PhantomData::default(),
        }
    }

    /// Returns the file descriptor.
    pub fn fd(&self) -> Fd {
        self.fd
    }

    /// Returns the file.
    pub fn file(&self) -> &mut dyn File {
        Activity::cur().files().get_raw(self.fd).unwrap()
    }

    /// Converts this file reference into a generic one
    pub fn into_generic(mut self) -> FileRef<dyn File> {
        self.close = false;
        FileRef::new_owned(self.fd)
    }
}

impl<T: ?Sized> Drop for FileRef<T> {
    fn drop(&mut self) {
        if self.close {
            Activity::cur().files().remove(self.fd);
        }
    }
}

impl<T: 'static> Deref for FileRef<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.file().as_any().downcast_ref().unwrap()
    }
}

impl<T: 'static> DerefMut for FileRef<T> {
    fn deref_mut(&mut self) -> &mut T {
        self.file().as_any_mut().downcast_mut().unwrap()
    }
}

impl<T: ?Sized + 'static> File for FileRef<T> {
    fn as_any(&self) -> &dyn Any {
        self.file().as_any()
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self.file().as_any_mut()
    }

    fn fd(&self) -> Fd {
        self.fd
    }

    fn set_fd(&mut self, fd: Fd) {
        self.file().set_fd(fd);
    }

    fn file_type(&self) -> u8 {
        self.file().file_type()
    }

    fn session(&self) -> Option<Selector> {
        self.file().session()
    }

    fn remove(&mut self) {
        self.file().remove();
    }

    fn stat(&self) -> Result<super::FileInfo, Error> {
        self.file().stat()
    }

    fn exchange_caps(
        &self,
        act: Selector,
        dels: &mut Vec<Selector>,
        max_sel: &mut Selector,
    ) -> Result<(), Error> {
        self.file().exchange_caps(act, dels, max_sel)
    }

    fn serialize(&self, s: &mut StateSerializer<'_>) {
        self.file().serialize(s);
    }

    fn is_blocking(&self) -> bool {
        self.file().is_blocking()
    }

    fn set_blocking(&mut self, blocking: bool) -> Result<(), Error> {
        self.file().set_blocking(blocking)
    }

    fn fetch_signal(&mut self) -> Result<bool, Error> {
        self.file().fetch_signal()
    }

    fn check_events(&mut self, events: FileEvent) -> bool {
        self.file().check_events(events)
    }
}

impl<T: ?Sized> Read for FileRef<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        self.file().read(buf)
    }
}

impl<T: ?Sized> Write for FileRef<T> {
    fn flush(&mut self) -> Result<(), Error> {
        self.file().flush()
    }

    fn sync(&mut self) -> Result<(), Error> {
        self.file().sync()
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        self.file().write(buf)
    }
}

impl<T: ?Sized> Seek for FileRef<T> {
    fn seek(&mut self, off: usize, whence: SeekMode) -> Result<usize, Error> {
        self.file().seek(off, whence)
    }
}

impl<T: ?Sized> Map for FileRef<T> {
    fn map(
        &self,
        pager: &Pager,
        virt: goff,
        off: usize,
        len: usize,
        prot: kif::Perm,
        flags: MapFlags,
    ) -> Result<(), Error> {
        self.file().map(pager, virt, off, len, prot, flags)
    }
}

impl<T: ?Sized> HashInput for FileRef<T> {
    fn hash_input(&mut self, sess: &HashSession, len: usize) -> Result<usize, Error> {
        self.file().hash_input(sess, len)
    }
}

impl<T: ?Sized> HashOutput for FileRef<T> {
    fn hash_output(&mut self, sess: &HashSession, len: usize) -> Result<usize, Error> {
        self.file().hash_output(sess, len)
    }
}

impl<T: ?Sized> fmt::Debug for FileRef<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FileRef[fd={}, file={:?}]", self.fd, self.file())
    }
}
