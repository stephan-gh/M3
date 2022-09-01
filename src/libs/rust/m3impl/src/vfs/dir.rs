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

use core::iter;

use crate::col::String;
use crate::errors::Error;
use crate::io::{read_object, Read};
use crate::mem;
use crate::vfs::{BufReader, FileRef, GenericFile, INodeId, OpenFlags, Seek, SeekMode, VFS};

/// Represents a directory entry.
#[derive(Debug)]
pub struct DirEntry {
    inode: INodeId,
    name: String,
}

impl DirEntry {
    /// Creates a new directory entry with given inode number and name.
    pub fn new(inode: INodeId, name: String) -> Self {
        DirEntry { inode, name }
    }

    /// Returns the inode number
    pub fn inode(&self) -> INodeId {
        self.inode
    }

    /// Returns the file name.
    pub fn file_name(&self) -> &str {
        &self.name
    }
}

/// An iterator to walk over a directory.
pub struct ReadDir {
    reader: BufReader<FileRef<GenericFile>>,
}

impl iter::Iterator for ReadDir {
    type Item = DirEntry;

    fn next(&mut self) -> Option<Self::Item> {
        #[derive(Default)]
        #[repr(C, packed)]
        struct M3FSDirEntry {
            inode: INodeId,
            name_len: u32,
            next: u32,
        }

        // read header
        let entry: M3FSDirEntry = match read_object(&mut self.reader) {
            Ok(obj) => obj,
            Err(_) => return None,
        };

        // read name
        let res = DirEntry::new(
            entry.inode,
            match self.reader.read_string(entry.name_len as usize) {
                Ok(s) => s,
                Err(_) => return None,
            },
        );

        // move to next entry
        let off = entry.next as usize - (mem::size_of::<M3FSDirEntry>() + entry.name_len as usize);
        if off != 0 && self.reader.seek(off, SeekMode::CUR).is_err() {
            return None;
        }

        Some(res)
    }
}

/// Returns an iterator for entries in the directory at `path`.
pub fn read_dir(path: &str) -> Result<ReadDir, Error> {
    let dir = VFS::open(path, OpenFlags::R)?;
    Ok(ReadDir {
        reader: BufReader::new(dir),
    })
}
