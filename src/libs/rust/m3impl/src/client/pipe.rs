/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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
use crate::cap::Selector;
use crate::client::ClientSession;
use crate::com::{opcodes, MemGate};
use crate::errors::Error;
use crate::kif::{CapRngDesc, CapType};
use crate::vfs::{File, GenericFile, OpenFlags};

/// Represents a session at the pipes server
///
/// The pipes server implements a uni-directional first-in-first-out communication channel with
/// multiple readers and writes and therefore provides the same semantics as anonymous pipes on
/// UNIX.
///
/// Note that [`IndirectPipe`](`crate::vfs::IndirectPipe`) provides a convenience layer on top of
/// this API.
pub struct Pipes {
    sess: ClientSession,
}

impl Pipes {
    /// Creates a new `Pipes` session at service with given name.
    pub fn new(name: &str) -> Result<Self, Error> {
        let sess = ClientSession::new(name)?;
        Ok(Pipes { sess })
    }

    /// Creates a new pipe using `mem` as shared memory for the data exchange.
    pub fn create_pipe(&self, mem: MemGate) -> Result<Pipe, Error> {
        let mem_size = mem.region()?.1;
        let crd = self.sess.obtain(
            1,
            |os| {
                os.push(opcodes::Pipe::OpenPipe);
                os.push(mem_size);
            },
            |_| Ok(()),
        )?;
        Pipe::new(mem, crd.start())
    }
}

/// Represents a pipe
///
/// A pipe allows to create *channels* that either write to the pipe or read from the pipe. To
/// exchange the data, the pipe requires memory, which is provided in form of a [`MemGate`].
pub struct Pipe {
    sess: ClientSession,
    mgate: MemGate,
}

impl Pipe {
    fn new(mem: MemGate, sel: Selector) -> Result<Self, Error> {
        let sess = ClientSession::new_owned_bind(sel);
        sess.delegate(
            CapRngDesc::new(CapType::Object, mem.sel(), 1),
            |os| {
                os.push(opcodes::Pipe::SetMem);
            },
            |_| Ok(()),
        )?;
        Ok(Pipe { sess, mgate: mem })
    }

    /// Returns the session's capability selector.
    pub fn sel(&self) -> Selector {
        self.sess.sel()
    }

    /// Returns the [`MemGate`] used for the data exchange
    pub fn memory(&self) -> &MemGate {
        &self.mgate
    }

    /// Creates a new channel for this pipe. If `read` is true, it is a read-end, otherwise a
    /// write-end.
    pub fn create_chan(&self, read: bool) -> Result<Box<dyn File>, Error> {
        let crd = self.sess.obtain(
            2,
            |os| {
                os.push(opcodes::Pipe::OpenChan);
                os.push(read);
            },
            |_| Ok(()),
        )?;
        let flags = if read {
            OpenFlags::R | OpenFlags::NEW_SESS
        }
        else {
            OpenFlags::W | OpenFlags::NEW_SESS
        };
        Ok(Box::new(GenericFile::new(flags, crd.start(), None)))
    }
}
