/*
 * Copyright (C) 2020-2021 Nils Asmussen, Barkhausen Institut
 * Copyright (C) 2019-2020, Tendsin Mende <tendsin@protonmail.com>
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

use crate::data::InodeNo;
use crate::ops::inodes;

use m3::col::Treap;
use m3::errors::Error;

pub struct OpenFile {
    appending: bool,
    deleted: bool,
    refs: usize,
}

impl OpenFile {
    pub fn new() -> Self {
        OpenFile {
            appending: false,
            deleted: false,
            refs: 1,
        }
    }

    pub fn appending(&self) -> bool {
        self.appending
    }

    pub fn set_appending(&mut self, new: bool) {
        self.appending = new;
    }
}

pub struct OpenFiles {
    files: Treap<InodeNo, OpenFile>,
}

impl OpenFiles {
    pub const fn new() -> Self {
        OpenFiles {
            files: Treap::new(),
        }
    }

    pub fn get_file_mut(&mut self, ino: InodeNo) -> Option<&mut OpenFile> {
        self.files.get_mut(&ino)
    }

    pub fn delete_file(&mut self, ino: InodeNo) -> Result<(), Error> {
        // create a request which executes the delete request on the FShandle
        if let Some(file) = self.get_file_mut(ino) {
            file.deleted = true;
        }
        else {
            inodes::free(ino)?;
        }
        Ok(())
    }

    pub fn add_sess(&mut self, ino: InodeNo) {
        // add reference to OpenFile instance or create new one
        if let Some(file) = self.get_file_mut(ino) {
            file.refs += 1;
        }
        else {
            self.files.insert(ino, OpenFile::new());
        }
    }

    pub fn remove_session(&mut self, ino: InodeNo) -> Result<(), Error> {
        let file = self.get_file_mut(ino).unwrap();

        // dereference OpenFile instance
        assert!(file.refs > 0);
        file.refs -= 1;

        // are there sessions left using the file?
        if file.refs == 0 {
            // if has the inode been deleted in the meantime, remove it
            if file.deleted {
                inodes::free(ino)?;
            }

            // remove OpenFile instance
            self.files.remove(&ino).unwrap();
        }

        Ok(())
    }
}
