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

use crate::cap::Selector;
use crate::cell::RefCell;
use crate::com::MemGate;
use crate::errors::Error;
use crate::rc::Rc;
use crate::serialize::Sink;
use crate::session::ClientSession;
use crate::vfs::{FileHandle, GenericFile, OpenFlags};

/// Represents a session at the pipes server.
pub struct Pipes {
    sess: ClientSession,
}

impl Pipes {
    /// Creates a new `Pipes` session at service with given name.
    pub fn new(name: &str) -> Result<Self, Error> {
        let sess = ClientSession::new(name)?;
        Ok(Pipes { sess })
    }

    /// Creates a new pipe using `mem` of `mem_size` bytes as shared memory for the data exchange.
    pub fn create_pipe(&self, mem: &MemGate, mem_size: usize) -> Result<Pipe, Error> {
        let crd = self.sess.obtain(
            2,
            |os| {
                os.push_word(mem_size as u64);
            },
            |_| Ok(()),
        )?;
        Pipe::new(mem, crd.start())
    }
}

/// Represents a pipe.
pub struct Pipe {
    sess: ClientSession,
}

impl Pipe {
    fn new(mem: &MemGate, sel: Selector) -> Result<Self, Error> {
        let sess = ClientSession::new_bind(sel);
        sess.delegate_obj(mem.sel())?;
        Ok(Pipe { sess })
    }

    /// Returns the session's capability selector.
    pub fn sel(&self) -> Selector {
        self.sess.sel()
    }

    /// Creates a new channel for this pipe. If `read` is true, it is a read-end, otherwise a
    /// write-end.
    pub fn create_chan(&self, read: bool) -> Result<FileHandle, Error> {
        let crd = self.sess.obtain(
            2,
            |os| {
                os.push_word(read as u64);
            },
            |_| Ok(()),
        )?;
        let flags = if read { OpenFlags::R } else { OpenFlags::W };
        Ok(Rc::new(RefCell::new(GenericFile::new(flags, crd.start()))))
    }
}
