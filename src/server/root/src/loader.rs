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
use m3::cap::Selector;
use m3::cfg::PAGE_BITS;
use m3::col::Vec;
use m3::com::{MemGate, VecSink, EP};
use m3::errors::{Code, Error};
use m3::goff;
use m3::io::{Read, Write};
use m3::kif::Perm;
use m3::pes::Mapper;
use m3::session::Pager;
use m3::syscalls;
use m3::util;
use m3::vfs;

use memory;

pub struct BootFile {
    mgate: MemGate,
    size: usize,
    pos: usize,
}

impl BootFile {
    pub fn new(sel: Selector, size: usize) -> Self {
        BootFile {
            mgate: MemGate::new_bind(sel),
            size,
            pos: 0,
        }
    }
}

impl vfs::File for BootFile {
    // not needed here
    fn fd(&self) -> vfs::Fd {
        0
    }

    fn set_fd(&mut self, _fd: vfs::Fd) {
    }

    fn evict(&mut self, _closing: bool) -> Option<EP> {
        None
    }

    fn close(&mut self) {
    }

    fn stat(&self) -> Result<vfs::FileInfo, Error> {
        let mut info = vfs::FileInfo::default();
        info.size = self.size;
        info.extents = 1;
        Ok(info)
    }

    fn file_type(&self) -> u8 {
        b'F'
    }

    fn exchange_caps(
        &self,
        _vpe: Selector,
        _dels: &mut Vec<Selector>,
        _max_sel: &mut Selector,
    ) -> Result<(), Error> {
        Err(Error::new(Code::NotSup))
    }

    fn serialize(&self, _s: &mut VecSink) {
        // not serializable
    }
}

impl vfs::Seek for BootFile {
    fn seek(&mut self, off: usize, whence: vfs::SeekMode) -> Result<usize, Error> {
        match whence {
            vfs::SeekMode::CUR => self.pos += off,
            vfs::SeekMode::SET => self.pos = off,
            vfs::SeekMode::END => self.pos = self.size,
            _ => panic!("Unexpected whence"),
        }
        Ok(self.pos)
    }
}

impl Read for BootFile {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        if self.pos >= self.size {
            Ok(0)
        }
        else {
            let amount = util::min(buf.len(), self.size - self.pos);
            self.mgate.read(&mut buf[0..amount], self.pos as goff)?;
            self.pos += amount;
            Ok(amount)
        }
    }
}

impl Write for BootFile {
    fn flush(&mut self) -> Result<(), Error> {
        // nothing to do
        Ok(())
    }

    fn write(&mut self, _buf: &[u8]) -> Result<usize, Error> {
        Err(Error::new(Code::NotSup))
    }
}

impl vfs::Map for BootFile {
    fn map(
        &self,
        _pager: &Pager,
        _virt: goff,
        _off: usize,
        _len: usize,
        _prot: Perm,
    ) -> Result<(), Error> {
        // not used
        Ok(())
    }
}

impl fmt::Debug for BootFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "BootFile[sel={}, size={:#x}, pos={:#x}]",
            self.mgate.sel(),
            self.size,
            self.pos
        )
    }
}

pub struct BootMapper {
    vpe_sel: Selector,
    mem_sel: Selector,
    has_virtmem: bool,
    allocs: Vec<memory::Allocation>,
}

impl BootMapper {
    pub fn new(vpe_sel: Selector, mem_sel: Selector, has_virtmem: bool) -> Self {
        BootMapper {
            vpe_sel,
            mem_sel,
            has_virtmem,
            allocs: Vec::new(),
        }
    }

    pub fn fetch_allocs(self) -> Vec<memory::Allocation> {
        self.allocs
    }
}

impl Mapper for BootMapper {
    fn map_file<'l>(
        &mut self,
        pager: Option<&'l Pager>,
        _file: &mut vfs::BufReader<vfs::FileRef>,
        foff: usize,
        virt: goff,
        len: usize,
        perm: Perm,
    ) -> Result<bool, Error> {
        if perm.contains(Perm::W) {
            // create new memory and copy data into it
            self.map_anon(pager, virt, len, perm)
        }
        else if self.has_virtmem {
            // map the memory of the boot module directly; therefore no initialization necessary
            syscalls::create_map(
                (virt >> PAGE_BITS) as Selector,
                self.vpe_sel,
                self.mem_sel,
                (foff >> PAGE_BITS) as Selector,
                (len >> PAGE_BITS) as Selector,
                perm,
            )
            .map(|_| false)
        }
        else {
            Ok(true)
        }
    }

    fn map_anon<'l>(
        &mut self,
        _pager: Option<&'l Pager>,
        virt: goff,
        len: usize,
        perm: Perm,
    ) -> Result<bool, Error> {
        if self.has_virtmem {
            let alloc = memory::get().allocate(len)?;
            let msel = memory::get().mem_cap(alloc.mod_id);

            syscalls::create_map(
                (virt >> PAGE_BITS) as Selector,
                self.vpe_sel,
                msel,
                (alloc.addr >> PAGE_BITS) as Selector,
                (len >> PAGE_BITS) as Selector,
                perm,
            )?;
            self.allocs.push(alloc);
            Ok(true)
        }
        else {
            // nothing to do
            Ok(true)
        }
    }
}
