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

use com::MemGate;
use errors::Error;
use rc::Rc;
use session::{Pipe, Pipes};
use vfs::Fd;
use vpe::VPE;

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
        Ok(IndirectPipe {
            rd_fd: VPE::cur().files().alloc(pipe.create_chan(true)?)?,
            wr_fd: VPE::cur().files().alloc(pipe.create_chan(false)?)?,
            _pipe: pipe,
        })
    }

    /// Returns the file descriptor of the reading side.
    pub fn reader_fd(&self) -> Fd {
        self.rd_fd
    }

    /// Closes the reading side.
    pub fn close_reader(&self) {
        VPE::cur().files().remove(self.rd_fd);
    }

    /// Returns the file descriptor of the writing side.
    pub fn writer_fd(&self) -> Fd {
        self.wr_fd
    }

    /// Closes the writing side.
    pub fn close_writer(&self) {
        VPE::cur().files().remove(self.wr_fd);
    }
}

impl Drop for IndirectPipe {
    fn drop(&mut self) {
        self.close_reader();
        self.close_writer();
    }
}
