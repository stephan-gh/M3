/*
 * Copyright (C) 2023 Nils Asmussen, Barkhausen Institut
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

use crate::boxed::Box;
use crate::client::ClientSession;
use crate::com::opcodes;
use crate::errors::Error;
use crate::tiles::Activity;
use crate::vfs::{FileRef, GenericFile, OpenFlags};

/// Represents a session at the virtual terminal server
pub struct VTerm {
    sess: ClientSession,
}

impl VTerm {
    /// Creates a new `VTerm` session at service with given name.
    pub fn new(name: &str) -> Result<Self, Error> {
        let sess = ClientSession::new(name)?;
        Ok(Self { sess })
    }

    /// Creates a new channel to the virtual terminal for either reading or writing.
    pub fn create_channel(&self, read: bool) -> Result<FileRef<GenericFile>, Error> {
        let crd = self.sess.obtain(
            2,
            |os| {
                os.push(opcodes::File::CloneFile);
                os.push(if read { 0 } else { 1 });
            },
            |_| Ok(()),
        )?;

        let flags = if read {
            OpenFlags::R | OpenFlags::NEW_SESS
        }
        else {
            OpenFlags::W | OpenFlags::NEW_SESS
        };
        let mut files = Activity::own().files();
        let fd = files.add(Box::new(GenericFile::new(flags, crd.start(), None)))?;
        Ok(files.get_as(fd).unwrap())
    }
}
