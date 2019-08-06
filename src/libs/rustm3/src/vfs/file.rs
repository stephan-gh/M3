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
use col::Vec;
use com::VecSink;
use core::fmt::Debug;
use errors::Error;
use goff;
use io::{Read, Write};
use kif;
use serialize::{Marshallable, Sink, Source, Unmarshallable};
use session::Pager;
use vfs::{BlockId, DevId, Fd, FileMode, INodeId};

int_enum! {
    /// The different seek modes.
    pub struct SeekMode : u32 {
        const SET       = 0x0;
        const CUR       = 0x1;
        const END       = 0x2;
    }
}

bitflags! {
    /// The flags to open files.
    pub struct OpenFlags : u32 {
        /// Opens the file for reading.
        const R         = 0b0000_0001;
        /// Opens the file for writing.
        const W         = 0b0000_0010;
        /// Opens the file for code execution.
        const X         = 0b0000_0100;
        /// Truncates the file on open.
        const TRUNC     = 0b0000_1000;
        /// Appends to the file.
        const APPEND    = 0b0001_0000;
        /// Creates the file if it doesn't exist.
        const CREATE    = 0b0010_0000;
        /// For benchmarking: only pretend to access the file's data.
        const NODATA    = 0b0100_0000;
        /// Do not create a file session, but store the file in the metadata session.
        ///
        /// Setting this flag improves the performance of [`VFS::open`] and [`VFS::close`], but
        /// does not allow to delegate the "file capability" to another VPE.
        const NOSESS    = 0b1000_0000;

        /// Opens the file for reading and writing.
        const RW        = Self::R.bits | Self::W.bits;
        /// Opens the file for reading and code execution.
        const RX        = Self::R.bits | Self::X.bits;
        /// Opens the file for reading, writing, and code execution.
        const RWX       = Self::R.bits | Self::W.bits | Self::X.bits;
    }
}

/// The file information that can be retrieved via `VFS::stat`.
#[derive(Copy, Clone, Debug, Default)]
#[repr(C, packed)]
pub struct FileInfo {
    pub devno: DevId,
    pub inode: INodeId,
    pub mode: FileMode,
    pub links: u32,
    pub size: usize,
    pub lastaccess: u32,
    pub lastmod: u32,
    pub blocksize: u32,
    // for debugging
    pub extents: u32,
    pub firstblock: BlockId,
}

impl Marshallable for FileInfo {
    fn marshall(&self, s: &mut dyn Sink) {
        s.push(&self.devno);
        s.push(&{ self.inode });
        s.push(&{ self.mode });
        s.push(&{ self.links });
        s.push(&{ self.size });
        s.push(&{ self.lastaccess });
        s.push(&{ self.lastmod });
        s.push(&{ self.blocksize });
        s.push(&{ self.extents });
        s.push(&{ self.firstblock });
    }
}

impl Unmarshallable for FileInfo {
    fn unmarshall(s: &mut dyn Source) -> Self {
        FileInfo {
            devno: s.pop_word() as DevId,
            inode: s.pop_word() as INodeId,
            mode: s.pop_word() as FileMode,
            links: s.pop_word() as u32,
            size: s.pop_word() as usize,
            lastaccess: s.pop_word() as u32,
            lastmod: s.pop_word() as u32,
            blocksize: s.pop_word() as u32,
            extents: s.pop_word() as u32,
            firstblock: s.pop_word() as BlockId,
        }
    }
}

/// Trait for files.
///
/// All files can be read, written, seeked and mapped into memory.
pub trait File: Read + Write + Seek + Map + Debug {
    /// Returns the file descriptor.
    fn fd(&self) -> Fd;
    /// Sets the file descriptor.
    fn set_fd(&mut self, fd: Fd);

    /// Evicts the file to be able to use it's memory endpoint for a different file.
    ///
    /// This is only used for file multiplexing.
    fn evict(&mut self);

    /// Closes the file.
    fn close(&mut self);

    /// Retrieves the file information.
    fn stat(&self) -> Result<FileInfo, Error>;

    /// Returns the type of the file implementation used for serialization.
    fn file_type(&self) -> u8;
    /// Exchanges the capabilities to provide `vpe` access to the file.
    fn exchange_caps(
        &self,
        vpe: Selector,
        dels: &mut Vec<Selector>,
        max_sel: &mut Selector,
    ) -> Result<(), Error>;
    /// Serializes this file into `s`.
    fn serialize(&self, s: &mut VecSink);
}

/// Trait for resources that are seekable.
pub trait Seek {
    /// Seeks to position `off`, using the given seek mode.
    ///
    /// If `whence` == SeekMode::SET, the position is set to `off`.
    /// If `whence` == SeekMode::CUR, the position is increased by `off`.
    /// If `whence` == SeekMode::END, the position is set to the end of the file.
    fn seek(&mut self, off: usize, whence: SeekMode) -> Result<usize, Error>;
}

/// Trait for resources that can be mapped into the virtual address space.
pub trait Map {
    /// Maps the region `off`..`off`+`len` of this file at address `virt` using the given pager and
    /// permissions.
    fn map(
        &self,
        pager: &Pager,
        virt: goff,
        off: usize,
        len: usize,
        prot: kif::Perm,
    ) -> Result<(), Error>;
}
