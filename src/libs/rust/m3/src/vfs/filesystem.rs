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
use core::any::Any;
use core::fmt;
use errors::Error;
use vfs::{FileHandle, FileInfo, FileMode, OpenFlags};

int_enum! {
    /// The file system operations.
    pub struct FSOperation : u64 {
        const STAT          = 0x7;
        const MKDIR         = 0x8;
        const RMDIR         = 0x9;
        const LINK          = 0xA;
        const UNLINK        = 0xB;
    }
}

/// Trait for file systems.
pub trait FileSystem: fmt::Debug {
    /// Returns an [`Any`] reference to downcast to the actual implementation of [`FileSystem`].
    fn as_any(&self) -> &dyn Any;

    /// Opens the file at `path` with given flags.
    fn open(&self, path: &str, flags: OpenFlags) -> Result<FileHandle, Error>;

    /// Retrieves the file information for the file at `path`.
    fn stat(&self, path: &str) -> Result<FileInfo, Error>;

    /// Creates a new directory with given permissions at `path`.
    fn mkdir(&self, path: &str, mode: FileMode) -> Result<(), Error>;
    /// Removes the directory at `path`, if it is empty.
    fn rmdir(&self, path: &str) -> Result<(), Error>;

    /// Links `new_path` to `old_path`.
    fn link(&self, old_path: &str, new_path: &str) -> Result<(), Error>;
    /// Removes the file at `path`.
    fn unlink(&self, path: &str) -> Result<(), Error>;

    /// Returns the type of the file system implementation used for serialization.
    fn fs_type(&self) -> u8;
    /// Exchanges the capabilities to provide `vpe` access to the file system.
    fn exchange_caps(
        &self,
        vpe: Selector,
        dels: &mut Vec<Selector>,
        max_sel: &mut Selector,
    ) -> Result<(), Error>;
    /// Serializes this file system into `s`.
    fn serialize(&self, s: &mut VecSink);
}
