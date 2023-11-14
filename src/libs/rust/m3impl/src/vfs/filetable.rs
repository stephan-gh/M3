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

use core::fmt;

use crate::boxed::Box;
use crate::cap::Selector;
use crate::cell::RefMut;
use crate::col::Vec;
use crate::errors::Error;
use crate::io::Serial;
use crate::serialize::{M3Deserializer, M3Serializer, VecSink};
use crate::tiles::{Activity, ChildActivity};
use crate::vfs::{File, FileRef, GenericFile};

/// A file descriptor
pub type Fd = usize;

/// The maximum number of files per [`FileTable`]
pub const INV_FD: usize = !0;

/// The table of open files
///
/// The table is indexed by a file descriptor. However, typical applications don't use the file
/// table directly, but use [`VFS::open`](`crate::vfs::VFS::open`) and [`FileRef`] to open and
/// access files.
#[derive(Default)]
pub struct FileTable {
    files: Vec<Option<Box<dyn File>>>,
}

impl FileTable {
    /// Adds the given file to this file table by allocating a new slot in the table
    pub fn add(&mut self, file: Box<dyn File>) -> Result<Fd, Error> {
        for (fd, cur_file) in self.files.iter().enumerate() {
            if cur_file.is_none() {
                self.set_raw(fd, file);
                return Ok(fd);
            }
        }

        self.files.push(Some(file));
        Ok(self.files.len() - 1)
    }

    /// Returns true if a file with given file descriptor exists
    pub fn exists(&self, fd: Fd) -> bool {
        fd < self.files.len() && self.files[fd].is_some()
    }

    /// Returns a reference to the file with given file descriptor
    pub fn get(&self, fd: Fd) -> Option<FileRef<dyn File>> {
        self.get_as(fd)
    }

    /// Returns a reference to the file with given file descriptor
    pub fn get_as<T: ?Sized>(&self, fd: Fd) -> Option<FileRef<T>> {
        if fd < self.files.len() {
            self.files[fd].as_ref().map(|_| FileRef::new(fd))
        }
        else {
            None
        }
    }

    /// Returns the file with given file descriptor
    pub(crate) fn get_raw(
        ftable: RefMut<'static, Self>,
        fd: Fd,
    ) -> Option<RefMut<'static, (dyn File + 'static)>> {
        if ftable.exists(fd) {
            Some(RefMut::map(ftable, |ft| {
                ft.files[fd].as_mut().unwrap().as_mut()
            }))
        }
        else {
            None
        }
    }

    /// Adds the given file to the table using the file descriptor `fd`, assuming that the file
    /// descriptor is not yet in use
    pub(crate) fn set_raw(&mut self, fd: Fd, mut file: Box<dyn File>) {
        if file.fd() == INV_FD {
            file.set_fd(fd);
        }

        if fd >= self.files.len() {
            self.files.reserve((fd + 1) - self.files.len());
            for _ in self.files.len()..fd {
                self.files.push(None);
            }
            self.files.push(Some(file));
        }
        else {
            assert!(self.files[fd].is_none());
            self.files[fd] = Some(file);
        }
    }

    /// Removes the file with given file descriptor from the table
    pub fn remove(&mut self, fd: Fd) {
        if let Some(ref mut f) = self.files[fd].take() {
            f.remove();
        }
    }

    pub(crate) fn delegate(&self, act: &ChildActivity) -> Result<Selector, Error> {
        let mut max_sel = 0;
        let files = act.files().clone();
        for (_cfd, pfd) in &files {
            if let Some(file) = self.files[*pfd].as_ref().map(|v| v.as_ref()) {
                let sel = file.delegate(act)?;
                max_sel = sel.max(max_sel);
            }
        }
        Ok(max_sel)
    }

    pub(crate) fn serialize(&self, files: &[(Fd, Fd)], s: &mut M3Serializer<VecSink<'_>>) {
        s.push(files.len());

        for (cfd, pfd) in files {
            if let Some(file) = self.files[*pfd].as_ref().map(|v| v.as_ref()) {
                s.push(*cfd);
                s.push(file.file_type());
                file.serialize(s);
            }
        }
    }

    pub(crate) fn unserialize(s: &mut M3Deserializer<'_>) -> FileTable {
        let mut ft = FileTable::default();

        let count = s.pop::<usize>().unwrap();
        for _ in 0..count {
            let fd: Fd = s.pop().unwrap();
            let file_type: u8 = s.pop().unwrap();
            ft.set_raw(fd, match file_type {
                b'F' => GenericFile::unserialize(s),
                b'S' => Box::new(Serial::new()),
                _ => panic!("Unexpected file type {}", file_type),
            });
        }

        ft
    }
}

impl fmt::Debug for FileTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "FileTable[")?;
        for (fd, file) in self.files.iter().enumerate() {
            if let Some(ref file_ref) = file {
                writeln!(f, "  {} -> {:?}", fd, file_ref)?;
            }
        }
        write!(f, "]")
    }
}

pub(crate) fn deinit() {
    let mut ft = Activity::own().files();
    for fd in 0..ft.files.len() {
        ft.remove(fd);
    }
}
