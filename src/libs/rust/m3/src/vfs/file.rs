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

use base::const_assert;
use bitflags::bitflags;
use core::fmt::Debug;

use crate::cap::Selector;
use crate::col::Vec;
use crate::errors::Error;
use crate::goff;
use crate::int_enum;
use crate::io::{Read, Write};
use crate::kif;
use crate::pes::StateSerializer;
use crate::session::{HashInput, HashOutput, MapFlags, Pager};
use crate::vfs::{BlockId, DevId, Fd, FileMode, INodeId};

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

        /// Opens the file for reading and writing.
        const RW        = Self::R.bits | Self::W.bits;
        /// Opens the file for reading and code execution.
        const RX        = Self::R.bits | Self::X.bits;
        /// Opens the file for reading, writing, and code execution.
        const RWX       = Self::R.bits | Self::W.bits | Self::X.bits;
    }
}

impl From<OpenFlags> for kif::Perm {
    fn from(flags: OpenFlags) -> Self {
        const_assert!(OpenFlags::R.bits() == kif::Perm::R.bits());
        const_assert!(OpenFlags::W.bits() == kif::Perm::W.bits());
        const_assert!(OpenFlags::X.bits() == kif::Perm::X.bits());
        kif::Perm::from_bits_truncate((flags & OpenFlags::RWX).bits() as u32)
    }
}

/// The file information that can be retrieved via [`VFS::stat`](crate::vfs::VFS::stat).
#[derive(Clone, Debug)]
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

/// The response for the stat call to m3fs.
// note that this is hand optimized, because stat is quite performance critical and the compiler
// seems to be unable to properly optimize everything with true marshalling/unmarshalling.
#[repr(C)]
pub struct StatResponse {
    pub error: u64,
    pub devno: u64,
    pub inode: u64,
    pub mode: u64,
    pub links: u64,
    pub size: u64,
    pub lastaccess: u64,
    pub lastmod: u64,
    pub blocksize: u64,
    pub extents: u64,
    pub firstblock: u64,
}

impl FileInfo {
    pub fn to_response(&self) -> StatResponse {
        StatResponse {
            error: 0,
            devno: self.devno as u64,
            inode: self.inode as u64,
            mode: self.mode as u64,
            links: self.links as u64,
            size: self.size as u64,
            lastaccess: self.lastaccess as u64,
            lastmod: self.lastmod as u64,
            blocksize: self.blocksize as u64,
            extents: self.extents as u64,
            firstblock: self.firstblock as u64,
        }
    }

    pub fn from_response(resp: &StatResponse) -> Result<Self, Error> {
        if resp.error != 0 {
            return Err(Error::from(resp.error as u32));
        }

        Ok(Self {
            devno: resp.devno as DevId,
            inode: resp.inode as INodeId,
            mode: resp.mode as FileMode,
            links: resp.links as u32,
            size: resp.size as usize,
            lastaccess: resp.lastaccess as u32,
            lastmod: resp.lastmod as u32,
            blocksize: resp.blocksize as u32,
            extents: resp.extents as u32,
            firstblock: resp.firstblock as BlockId,
        })
    }
}

/// Trait for files.
///
/// All files can be read, written, seeked and mapped into memory.
pub trait File: Read + Write + Seek + Map + Debug + HashInput + HashOutput {
    /// Returns the file descriptor.
    fn fd(&self) -> Fd;
    /// Sets the file descriptor.
    fn set_fd(&mut self, fd: Fd);

    /// Returns the session selector, if any
    fn session(&self) -> Option<Selector>;

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
    fn serialize(&self, s: &mut StateSerializer);
}

/// Trait for resources that are seekable.
pub trait Seek {
    /// Seeks to position `off`, using the given seek mode.
    ///
    /// If `whence` == [`SeekMode::SET`], the position is set to `off`.
    /// If `whence` == [`SeekMode::CUR`], the position is increased by `off`.
    /// If `whence` == [`SeekMode::END`], the position is set to the end of the file.
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
        flags: MapFlags,
    ) -> Result<(), Error>;
}
