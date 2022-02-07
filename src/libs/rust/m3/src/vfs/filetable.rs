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

use core::{fmt, mem};

use crate::cap::Selector;
use crate::cell::RefCell;
use crate::col::Vec;
use crate::errors::Error;
use crate::io::Serial;
use crate::rc::Rc;
use crate::serialize::Source;
use crate::tiles::{Activity, StateSerializer};
use crate::vfs::{File, FileRef, GenericFile};

/// A file descriptor
pub type Fd = usize;

/// The maximum number of files per [`FileTable`].
pub const INV_FD: usize = !0;

/// A reference to a file.
pub type FileHandle = Rc<RefCell<dyn File>>;

/// The table of open files.
#[derive(Default)]
pub struct FileTable {
    files: Vec<Option<FileHandle>>,
}

impl FileTable {
    /// Adds the given file to this file table by allocating a new slot in the table.
    pub fn add(&mut self, file: FileHandle) -> Result<FileRef, Error> {
        self.alloc(file.clone()).map(|fd| FileRef::new(file, fd))
    }

    /// Allocates a new slot in the file table and returns its file descriptor.
    pub fn alloc(&mut self, file: FileHandle) -> Result<Fd, Error> {
        for (fd, cur_file) in self.files.iter().enumerate() {
            if cur_file.is_none() {
                self.set(fd, file);
                return Ok(fd);
            }
        }

        self.files.push(Some(file));
        Ok(self.files.len() - 1)
    }

    /// Returns a reference to the file with given file descriptor. The file will be closed as soon
    /// as the reference is dropped.
    pub fn get_ref(&self, fd: Fd) -> Option<FileRef> {
        if fd < self.files.len() {
            self.files[fd].as_ref().map(|f| FileRef::new(f.clone(), fd))
        }
        else {
            None
        }
    }

    /// Returns the file with given file descriptor.
    pub fn get(&self, fd: Fd) -> Option<FileHandle> {
        if fd < self.files.len() {
            self.files[fd].as_ref().cloned()
        }
        else {
            None
        }
    }

    /// Adds the given file to the table using the file descriptor `fd`, assuming that the file
    /// descriptor is not yet in use.
    pub fn set(&mut self, fd: Fd, file: FileHandle) {
        if file.borrow().fd() == INV_FD {
            file.borrow_mut().set_fd(fd);
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

    /// Removes the file with given file descriptor from the table.
    pub fn remove(&mut self, fd: Fd) {
        if let Some(ref mut f) = mem::replace(&mut self.files[fd], None) {
            f.borrow_mut().close();
        }
    }

    pub(crate) fn collect_caps(
        &self,
        act: Selector,
        dels: &mut Vec<Selector>,
        max_sel: &mut Selector,
    ) -> Result<(), Error> {
        for file in self.files.iter().flatten() {
            file.borrow().exchange_caps(act, dels, max_sel)?;
        }
        Ok(())
    }

    pub(crate) fn serialize(&self, s: &mut StateSerializer) {
        let count = self.files.iter().filter(|&f| f.is_some()).count();
        s.push_word(count as u64);

        for (fd, file) in self.files.iter().enumerate() {
            if let Some(ref file_ref) = file {
                let file_obj = file_ref.borrow();
                s.push_word(fd as u64);
                s.push_word(file_obj.file_type() as u64);
                file_obj.serialize(s);
            }
        }
    }

    pub(crate) fn unserialize(s: &mut Source) -> FileTable {
        let mut ft = FileTable::default();

        let count = s.pop().unwrap();
        for _ in 0..count {
            let fd: Fd = s.pop().unwrap();
            let file_type: u8 = s.pop().unwrap();
            ft.set(fd, match file_type {
                b'F' => GenericFile::unserialize(s),
                b'S' => Rc::new(RefCell::new(Serial::new())),
                _ => panic!("Unexpected file type {}", file_type),
            });
        }

        ft
    }
}

impl fmt::Debug for FileTable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
    let ft = Activity::cur().files();
    for fd in 0..ft.files.len() {
        ft.remove(fd);
    }
}
