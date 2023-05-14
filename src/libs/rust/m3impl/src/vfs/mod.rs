/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2021 Nils Asmussen, Barkhausen Institut
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

//! The virtual file system (VFS)
//!
//! The VFS provides access to file systems and files. All file systems implement the [`FileSystem`]
//! trait, whereas files implement the [`File`] trait. The former is currently only implemented by
//! [`M3FS`](`crate::client::M3FS`) as this is the only available file system on M³. The latter is
//! implemented by multiple types:
//! - files that implement the *file protocol*: [`GenericFile`]
//! - sockets: [`UdpSocket`](`crate::net::UdpSocket`), [`TcpSocket`](`crate::net::TcpSocket`), and
//!   [`RawSocket`](`crate::net::RawSocket`)
//! - file references: [`FileRef`]
//!
//! # Accessing files and directories
//!
//! [`VFS`] offers the application-facing API to open files, create directories, rename files, etc.
//! For example, a file can be opened and read in the following way:
//!
//! ```
//! let mut file = VFS::open("/dir/myfile", OpenFlags::R).unwrap();
//! let content = file.read_to_string().unwrap();
//! println!("content: {}", content);
//! ```
//!
//! Similarly, a directory can be listed as follows:
//! ```
//! for entry in VFS::read_dir("/mydir").unwrap() {
//!   println!("Found entry {}", entry.file_name());
//! }
//! ```
//!
//! # File references
//!
//! When opening a file or directory, applications do not work directly with an implementation of
//! [`File`], but indirectly via [`FileRef`]. As the name implies, [`FileRef`] holds a reference to
//! the file (file descriptor) and provides access to all methods from [`File`] by implementing the
//! trait itself. Most importantly, [`FileRef`] closes the file automatically on drop.
//!
//! # FileTable and MountTable
//!
//! [`FileTable`] and [`MountTable`] hold all open files and mount points, respectively. Files are
//! found via an index into the table called *file descriptor*, whereas mount points are found by
//! file path. Therefore, opening a file via [`VFS::open`] expects a file path that is first
//! resolved via [`MountTable`] to the file system at the corresponding mount path and the remaining
//! path within the file system. [`VFS::open`] then refers to the found [`FileSystem`]
//! implementation to open the file on the server side. Finally, a [`File`] instance is created and
//! inserted into the [`FileTable`]. The caller of [`VFS::open`] receives a [`FileRef`] that holds
//! the file descriptor and provides access to the methods of [`File`] by borrowing the [`File`]
//! object from [`FileTable`] during the call.
//!
//! # Delegation of files and mount points
//!
//! [`FileTable`] and [`MountTable`] are not used directly, but indirectly through
//! [`OwnActivity`](`crate::tiles::OwnActivity`), which in turn is used by [`FileRef`] to get to the
//! file it references. The reason is that both files and file systems can be delegated to
//! [`ChildActivity`](`crate::tiles::ChildActivity`)s. Therefore,
//! [`ChildActivity`](`crate::tiles::ChildActivity`) keeps a list of files and mount points to
//! delegate and uses [`FileTable`] and [`MountTable`] before starting the activity to perform the
//! delegation. [`OwnActivity`](`crate::tiles::OwnActivity`) uses [`FileTable`] and [`MountTable`]
//! to receive these delegations upon application start and provides access to them afterwards.

mod bufio;
mod dir;
mod file;
mod fileref;
mod filesystem;
mod filetable;
mod genericfile;
mod indirpipe;
mod mounttable;
#[allow(clippy::module_inception)]
mod vfs;
mod waiter;

/// A device ID
pub type DevId = u8;
/// An inode ID
pub type INodeId = u32;
/// A block ID
pub type BlockId = u32;

pub use self::bufio::{BufReader, BufWriter};
pub use self::dir::{DirEntry, ReadDir};
pub use self::file::{File, FileEvent, FileInfo, FileMode, Map, OpenFlags, Seek, SeekMode, TMode};
pub use self::fileref::FileRef;
pub use self::filesystem::FileSystem;
pub(crate) use self::filetable::INV_FD;
pub use self::filetable::{Fd, FileTable};
pub use self::genericfile::GenericFile;
pub use self::indirpipe::IndirectPipe;
pub use self::mounttable::{FSHandle, MountTable};
pub use self::waiter::FileWaiter;

/// The VFS module provides the application-facing API for files and file systems
///
/// Like in other systems, files are organized hierarchically and file systems are mounted at
/// arbitrary points within this hierachie. However, file systems are mounted locally within an
/// application and can selectively be delegated to
/// [`ChildActivity`](`crate::tiles::ChildActivity`)s, if desired.
///
/// All paths are resolved by first finding the responsible file systems via the mount points stored
/// in [`MountTable`], followed by finding the file associated with the path within the found file
/// system. Furthermore, the environment variable `PWD` (see [`VFS::cwd`]) is prepended to relative
/// paths.
#[allow(non_snake_case)]
pub mod VFS {
    pub use crate::vfs::vfs::*;
}

pub(crate) fn deinit() {
    filetable::deinit();
}
