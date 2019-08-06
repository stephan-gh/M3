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
use cell::StaticCell;
use col::Vec;
use errors::{Code, Error};
use rc::Rc;
use session::M3FS;
use vfs::{FSHandle, FileInfo, FileMode, FileRef, OpenFlags};
use vpe::VPE;

struct ResEps {
    fs: FSHandle,
    sel: Selector,
    count: u32,
    used: u32,
}

impl ResEps {
    pub fn new(fs: FSHandle, sel: Selector, count: u32) -> Self {
        ResEps {
            fs,
            sel,
            count,
            used: 0,
        }
    }

    pub fn has_ep(&self, ep: Selector) -> bool {
        ep >= self.sel && ep < self.sel + self.count
    }

    pub fn alloc_ep(&mut self) -> Option<Selector> {
        for i in 0..self.count {
            if self.used & (1 << i) == 0 {
                self.used |= 1 << i;
                return Some(self.sel + i);
            }
        }
        None
    }

    pub fn free_ep(&mut self, ep: Selector) {
        self.used &= !(1 << (ep - self.sel));
    }
}

static RES_EPS: StaticCell<Vec<ResEps>> = StaticCell::new(Vec::new());

/// Delegates the given endpoint capabilities to the file system mounted at `path`.
///
/// These endpoints will afterwards be used for files that are opened with [`OpenFlags::NOSESS`],
/// which are less expensive to open/close.
pub fn delegate_eps(path: &str, first: Selector, count: u32) -> Result<(), Error> {
    with_path(path, |fs, _| {
        fs.borrow_mut().delegate_eps(first, count)?;
        RES_EPS
            .get_mut()
            .push(ResEps::new(fs.clone(), first, count));
        Ok(())
    })
}

pub(crate) fn alloc_ep(fs: FSHandle) -> Result<(u32, Selector), Error> {
    for r in RES_EPS.get_mut() {
        if Rc::ptr_eq(&r.fs, &fs) {
            let ep = r.alloc_ep().ok_or_else(|| Error::new(Code::NoSpace))?;
            let idx = ep - r.sel;
            return Ok((idx, ep));
        }
    }
    Err(Error::new(Code::InvArgs))
}

pub(crate) fn free_ep(ep: Selector) {
    for r in RES_EPS.get_mut() {
        if r.has_ep(ep) {
            r.free_ep(ep);
            break;
        }
    }
}

/// Mounts the file system of type `fstype` at `path`, creating a session at `service`.
pub fn mount(path: &str, fstype: &str, service: &str) -> Result<(), Error> {
    let fsobj = match fstype {
        "m3fs" => M3FS::new(service)?,
        _ => return Err(Error::new(Code::InvArgs)),
    };
    VPE::cur().mounts().add(path, fsobj)
}

/// Umounts the file system mounted at `path`.
pub fn unmount(path: &str) -> Result<(), Error> {
    VPE::cur().mounts().remove(path)
}

fn with_path<F, R>(path: &str, func: F) -> Result<R, Error>
where
    F: Fn(&FSHandle, usize) -> Result<R, Error>,
{
    let (fs, pos) = VPE::cur().mounts().resolve(path)?;
    func(&fs, pos)
}

/// Opens the file at `path` with given flags.
pub fn open(path: &str, flags: OpenFlags) -> Result<FileRef, Error> {
    with_path(path, |fs, pos| {
        let file = fs.borrow_mut().open(&path[pos..], flags)?;
        VPE::cur().files().add(file)
    })
}

/// Retrieves the file information from the file at `path`.
pub fn stat(path: &str) -> Result<FileInfo, Error> {
    with_path(path, |fs, pos| fs.borrow().stat(&path[pos..]))
}

/// Creates a directory with permissions `mode` at `path`.
pub fn mkdir(path: &str, mode: FileMode) -> Result<(), Error> {
    with_path(path, |fs, pos| fs.borrow().mkdir(&path[pos..], mode))
}

/// Removes the directory at `path`, if it is empty.
pub fn rmdir(path: &str) -> Result<(), Error> {
    with_path(path, |fs, pos| fs.borrow().rmdir(&path[pos..]))
}

/// Creates a link at `new` to `old`.
pub fn link(old: &str, new: &str) -> Result<(), Error> {
    let (fs1, pos1) = VPE::cur().mounts().resolve(old)?;
    let (fs2, pos2) = VPE::cur().mounts().resolve(new)?;
    if !Rc::ptr_eq(&fs1, &fs2) {
        return Err(Error::new(Code::XfsLink));
    }
    #[allow(clippy::let_and_return)] // is required because of fs1.borrow()'s lifetime
    let res = fs1.borrow().link(&old[pos1..], &new[pos2..]);
    res
}

/// Removes the file at `path`.
pub fn unlink(path: &str) -> Result<(), Error> {
    with_path(path, |fs, pos| fs.borrow().unlink(&path[pos..]))
}
