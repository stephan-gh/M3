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

//! The virtual file system.

mod bufio;
mod dir;
mod file;
mod fileref;
mod filesystem;
mod filetable;
mod genericfile;
mod indirpipe;
mod mounttable;
mod vfs;

/// File permissions.
pub type FileMode = u16;
/// A device ID
pub type DevId = u8;
/// An inode ID
pub type INodeId = u32;
/// A block ID
pub type BlockId = u32;

pub use self::bufio::{BufReader, BufWriter};
pub use self::dir::{DirEntry, ReadDir, read_dir};
pub use self::file::{File, FileInfo, Map, OpenFlags, SeekMode, Seek};
pub use self::fileref::FileRef;
pub use self::filesystem::{FileSystem, FSOperation};
pub use self::filetable::{Fd, FileHandle, FileTable};
pub use self::genericfile::{GenericFile, GenFileOp};
pub use self::indirpipe::IndirectPipe;
pub use self::mounttable::{FSHandle, MountTable};

#[allow(non_snake_case)]
pub mod VFS {
    pub use vfs::vfs::*;
}

pub(crate) fn deinit() {
    filetable::deinit();
}
