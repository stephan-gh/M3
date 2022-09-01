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

use crate::com::MemGate;
use crate::errors::Error;
use crate::rc::Rc;
use crate::session::{Pipe, Pipes};
use crate::tiles::Activity;
use crate::vfs::{Fd, FileRef, GenericFile};

/// A uni-directional channel between potentially multiple readers and writers.
pub struct IndirectPipe {
    _pipe: Rc<Pipe>,
    rd_fd: Fd,
    wr_fd: Fd,
}

impl IndirectPipe {
    /// Creates a new pipe at pipe service `pipes` using `mem` as the shared memory of `mem_size`
    /// bytes.
    pub fn new(pipes: &Pipes, mem: &MemGate, mem_size: usize) -> Result<Self, Error> {
        let pipe = Rc::new(pipes.create_pipe(mem, mem_size)?);
        let mut files = Activity::own().files();
        let rd_fd = files.add(pipe.create_chan(true)?)?;
        let wr_fd = files.add(pipe.create_chan(false)?)?;
        Ok(IndirectPipe {
            rd_fd,
            wr_fd,
            _pipe: pipe,
        })
    }

    /// Returns the file for the reading side.
    pub fn reader(&self) -> Option<FileRef<GenericFile>> {
        Activity::own().files().get_as(self.rd_fd)
    }

    /// Closes the reading side.
    pub fn close_reader(&self) {
        Activity::own().files().remove(self.rd_fd);
    }

    /// Returns the file for the writing side.
    pub fn writer(&self) -> Option<FileRef<GenericFile>> {
        Activity::own().files().get_as(self.wr_fd)
    }

    /// Closes the writing side.
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
