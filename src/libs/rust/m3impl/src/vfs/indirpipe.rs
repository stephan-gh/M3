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

use crate::client::{Pipe, Pipes};
use crate::com::MemGate;
use crate::errors::Error;
use crate::rc::Rc;
use crate::tiles::Activity;
use crate::vfs::{Fd, FileRef, GenericFile};

/// A uni-directional communication channel
///
/// The `IndirectPipe` provides a uni-directional first-in-first-out communication channel with
/// multiple readers and writes and therefore provides the same semantics as anonymous pipes on
/// UNIX. It is called indirect, because the communication between writer and reader happens
/// indirectly via the pipe server.
pub struct IndirectPipe {
    _pipe: Rc<Pipe>,
    rd_fd: Fd,
    wr_fd: Fd,
}

impl IndirectPipe {
    /// Creates a new pipe at the service with given name
    ///
    /// The argument `mem` specifies the memory region that should be used to exchange the data.
    /// Besides creating the pipe itself, two channels are created, one for reading and one for
    /// writing. The methods [`IndirectPipe::reader`] and [`IndirectPipe::writer`] provide access to
    /// these channels. In case one or both channels are delegated to another activity, the channel
    /// can be closed via [`IndirectPipe::close_reader`] or [`IndirectPipe::close_writer`].
    pub fn new(pipes: &Pipes, mem: MemGate) -> Result<Self, Error> {
        let pipe = Rc::new(pipes.create_pipe(mem)?);
        let mut files = Activity::own().files();
        let rd_fd = files.add(pipe.create_chan(true)?)?;
        let wr_fd = files.add(pipe.create_chan(false)?)?;
        Ok(IndirectPipe {
            rd_fd,
            wr_fd,
            _pipe: pipe,
        })
    }

    /// Returns the file for the reading side
    pub fn reader(&self) -> Option<FileRef<GenericFile>> {
        Activity::own().files().get_as(self.rd_fd)
    }

    /// Closes the reading side
    pub fn close_reader(&self) {
        Activity::own().files().remove(self.rd_fd);
    }

    /// Returns the file for the writing side
    pub fn writer(&self) -> Option<FileRef<GenericFile>> {
        Activity::own().files().get_as(self.wr_fd)
    }

    /// Closes the writing side
    pub fn close_writer(&self) {
        Activity::own().files().remove(self.wr_fd);
    }
}

impl Drop for IndirectPipe {
    fn drop(&mut self) {
        self.close_reader();
        self.close_writer();
    }
}
