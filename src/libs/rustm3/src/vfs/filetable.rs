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

use cap::Selector;
use cell::RefCell;
use col::Vec;
use com::{SliceSource, VecSink, EP};
use core::{fmt, mem};
use errors::{Code, Error};
use io::Serial;
use pes::VPE;
use rc::Rc;
use serialize::Sink;
use vfs::{File, FileRef, GenericFile};

/// A file descriptor
pub type Fd = usize;

/// The maximum number of endpoints that can be dedicated to files.
pub const MAX_EPS: usize = 4;
/// The maximum number of files per [`FileTable`].
pub const MAX_FILES: usize = 32;

/// A reference to a file.
pub type FileHandle = Rc<RefCell<dyn File>>;

/// The table of open files.
#[derive(Default)]
pub struct FileTable {
    free_eps: Vec<EP>,
    file_ep_victim: usize,
    used_ep_count: usize,
    used_eps: [Option<Fd>; MAX_EPS],
    files: [Option<FileHandle>; MAX_FILES],
}

impl FileTable {
    /// Adds the given file to this file table by allocating a new slot in the table.
    pub fn add(&mut self, file: FileHandle) -> Result<FileRef, Error> {
        self.alloc(file.clone()).map(|fd| FileRef::new(file, fd))
    }

    /// Allocates a new slot in the file table and returns its file descriptor.
    pub fn alloc(&mut self, file: FileHandle) -> Result<Fd, Error> {
        for fd in 0..MAX_FILES {
            if self.files[fd].is_none() {
                self.set(fd, file);
                return Ok(fd);
            }
        }
        Err(Error::new(Code::NoSpace))
    }

    /// Returns a reference to the file with given file descriptor. The file will be closed as soon
    /// as the reference is dropped.
    pub fn get_ref(&self, fd: Fd) -> Option<FileRef> {
        self.files[fd].as_ref().map(|f| FileRef::new(f.clone(), fd))
    }

    /// Returns the file with given file descriptor.
    pub fn get(&self, fd: Fd) -> Option<FileHandle> {
        self.files[fd].as_ref().cloned()
    }

    /// Adds the given file to the table using the file descriptor `fd`, assuming that the file
    /// descriptor is not yet in use.
    pub fn set(&mut self, fd: Fd, file: FileHandle) {
        assert!(self.files[fd].is_none());
        if file.borrow().fd() == MAX_FILES {
            file.borrow_mut().set_fd(fd);
        }
        self.files[fd] = Some(file);
    }

    /// Removes the file with given file descriptor from the table.
    pub fn remove(&mut self, fd: Fd) {
        let find_file_ep = |files: &[Option<Fd>], fd| -> Option<usize> {
            for (i, f) in files.iter().enumerate() {
                match f {
                    Some(id) if *id == fd => return Some(i),
                    _ => continue,
                }
            }
            None
        };

        if let Some(ref mut f) = mem::replace(&mut self.files[fd], None) {
            if let Some(ep) = f.borrow_mut().evict(true) {
                if ep.valid() {
                    self.free_eps.push(ep);
                }
            }

            // remove from multiplexing table
            if let Some(idx) = find_file_ep(&self.used_eps, fd) {
                log!(FILES, "FileEPs[{}] = --", idx);
                self.used_eps[idx] = None;
                self.used_ep_count -= 1;
            }

            f.borrow_mut().close();
        }
    }

    pub(crate) fn request_ep(&mut self, fd: Fd) -> Result<EP, Error> {
        if self.used_ep_count < MAX_EPS {
            let ep = if let Some(ep) = self.free_eps.pop() {
                Ok(ep)
            }
            else {
                EP::new()
            };

            if let Ok(ep) = ep {
                for i in 0..MAX_EPS {
                    if self.used_eps[i].is_none() {
                        log!(FILES, "FileEPs[{}] = EP:{},FD:{}", i, ep.id().unwrap(), fd);

                        self.used_eps[i] = Some(fd);
                        self.used_ep_count += 1;
                        return Ok(ep);
                    }
                }
            }
        }

        // TODO be smarter here
        let mut i = self.file_ep_victim;
        for _ in 0..MAX_EPS {
            if let Some(ofd) = self.used_eps[i] {
                let file = self.files[ofd].as_ref().unwrap();
                let ep = file.borrow_mut().evict(false).unwrap();

                log!(
                    FILES,
                    "FileEPs[{}] = EP:{},FD: switching from {} to {}",
                    i,
                    ep.id().unwrap(),
                    ofd,
                    fd
                );

                self.used_eps[i] = Some(fd);
                self.file_ep_victim = (i + 1) % MAX_EPS;
                return Ok(ep);
            }

            i = (i + 1) % MAX_EPS;
        }

        Err(Error::new(Code::NoSpace))
    }

    pub(crate) fn collect_caps(
        &self,
        vpe: Selector,
        dels: &mut Vec<Selector>,
        max_sel: &mut Selector,
    ) -> Result<(), Error> {
        for fd in 0..MAX_FILES {
            if let Some(ref f) = self.files[fd] {
                f.borrow().exchange_caps(vpe, dels, max_sel)?;
            }
        }
        Ok(())
    }

    pub(crate) fn serialize(&self, s: &mut VecSink) {
        let count = self.files.iter().filter(|&f| f.is_some()).count();
        s.push(&count);

        for fd in 0..MAX_FILES {
            if let Some(ref f) = self.files[fd] {
                let file = f.borrow();
                s.push(&fd);
                s.push(&file.file_type());
                file.serialize(s);
            }
        }
    }

    pub(crate) fn unserialize(s: &mut SliceSource) -> FileTable {
        let mut ft = FileTable::default();

        let count = s.pop();
        for _ in 0..count {
            let fd: Fd = s.pop();
            let file_type: u8 = s.pop();
            ft.set(fd, match file_type {
                b'F' => GenericFile::unserialize(s),
                b'S' => Rc::new(RefCell::new(Serial::default())),
                _ => panic!("Unexpected file type {}", file_type),
            });
        }

        ft
    }
}

impl fmt::Debug for FileTable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "FileTable[")?;
        for fd in 0..MAX_FILES {
            if let Some(ref file) = self.files[fd] {
                writeln!(f, "  {} -> {:?}", fd, file)?;
            }
        }
        write!(f, "]")
    }
}

pub(crate) fn deinit() {
    let ft = VPE::cur().files();
    for fd in 0..MAX_FILES {
        ft.remove(fd);
    }
}
