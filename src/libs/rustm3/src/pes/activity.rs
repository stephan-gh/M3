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

/// The mapper trait is used to map the memory of an activity before running it.

use com::MemGate;
use env;
use errors::Error;
use goff;
use io::Read;
use kif;
use pes::VPE;
use session::Pager;
use syscalls;
use util;
use vfs::{BufReader, FileRef, Map, Seek, SeekMode};

pub trait Mapper {
    /// Maps the given file to `virt`..`virt`+`len` with given permissions.
    fn map_file<'l>(
        &mut self,
        pager: Option<&'l Pager>,
        file: &mut BufReader<FileRef>,
        foff: usize,
        virt: goff,
        len: usize,
        perm: kif::Perm,
    ) -> Result<bool, Error>;

    /// Maps anonymous memory to `virt`..`virt`+`len` with given permissions.
    fn map_anon<'l>(
        &mut self,
        pager: Option<&'l Pager>,
        virt: goff,
        len: usize,
        perm: kif::Perm,
    ) -> Result<bool, Error>;

    /// Initializes the memory at `virt`..`memsize` by loading `fsize` bytes from the given file at
    /// `foff` and zero'ing the remaining space.
    ///
    /// The argument `buf` can be used as a buffer and `mem` refers to the address space of the VPE.
    #[allow(clippy::too_many_arguments)]
    fn init_mem(
        &self,
        buf: &mut [u8],
        mem: &MemGate,
        file: &mut BufReader<FileRef>,
        foff: usize,
        fsize: usize,
        virt: goff,
        memsize: usize,
    ) -> Result<(), Error> {
        file.seek(foff, SeekMode::SET)?;

        let mut count = fsize;
        let mut segoff = virt as usize;
        while count > 0 {
            let amount = util::min(count, buf.len());
            let amount = file.read(&mut buf[0..amount])?;

            mem.write(&buf[0..amount], segoff as goff)?;

            count -= amount;
            segoff += amount;
        }

        self.clear_mem(buf, mem, segoff, (memsize - fsize) as usize)
    }

    /// Overwrites `virt`..`virt`+`len` with zeros in the address space given by `mem`.
    ///
    /// The argument `buf` can be used as a buffer.
    fn clear_mem(
        &self,
        buf: &mut [u8],
        mem: &MemGate,
        mut virt: usize,
        mut len: usize,
    ) -> Result<(), Error> {
        if len == 0 {
            return Ok(());
        }

        for it in buf.iter_mut() {
            *it = 0;
        }

        while len > 0 {
            let amount = util::min(len, buf.len());
            mem.write(&buf[0..amount], virt as goff)?;
            len -= amount;
            virt += amount;
        }

        Ok(())
    }
}

/// The default implementation of the [`Mapper`] trait.
pub struct DefaultMapper {
    has_virtmem: bool,
}

impl DefaultMapper {
    /// Creates a new `DefaultMapper`.
    pub fn new(has_virtmem: bool) -> Self {
        DefaultMapper { has_virtmem }
    }
}

impl Mapper for DefaultMapper {
    fn map_file<'l>(
        &mut self,
        pager: Option<&'l Pager>,
        file: &mut BufReader<FileRef>,
        foff: usize,
        virt: goff,
        len: usize,
        perm: kif::Perm,
    ) -> Result<bool, Error> {
        if let Some(pg) = pager {
            file.get_ref().map(pg, virt, foff, len, perm).map(|_| false)
        }
        else if self.has_virtmem {
            // TODO handle that case
            unimplemented!();
        }
        else {
            Ok(true)
        }
    }

    fn map_anon<'l>(
        &mut self,
        pager: Option<&'l Pager>,
        virt: goff,
        len: usize,
        perm: kif::Perm,
    ) -> Result<bool, Error> {
        if let Some(pg) = pager {
            pg.map_anon(virt, len, perm).map(|_| false)
        }
        else if self.has_virtmem {
            // TODO handle that case
            unimplemented!();
        }
        else {
            Ok(true)
        }
    }
}

/// Represents an activity that is run on a [`VPE`].
pub trait Activity {
    /// Returns a reference to the VPE.
    fn vpe(&self) -> &VPE;
    /// Returns a mutable reference to the VPE.
    fn vpe_mut(&mut self) -> &mut VPE;

    /// Starts the activity.
    fn start(&self) -> Result<(), Error> {
        syscalls::vpe_ctrl(self.vpe().sel(), kif::syscalls::VPEOp::START, 0).map(|_| ())
    }

    /// Stops the activity.
    fn stop(&self) -> Result<(), Error> {
        syscalls::vpe_ctrl(self.vpe().sel(), kif::syscalls::VPEOp::STOP, 0).map(|_| ())
    }

    /// Waits until the activity exits and returns the error code.
    fn wait(&self) -> Result<i32, Error> {
        syscalls::vpe_wait(&[self.vpe().sel()], 0).map(|r| r.1)
    }

    /// Starts an asynchronous wait for the activity, using the given event for the upcall.
    fn wait_async(&self, event: u64) -> Result<i32, Error> {
        syscalls::vpe_wait(&[self.vpe().sel()], event).map(|r| r.1)
    }
}

/// The activity for [`VPE::run`].
pub struct ClosureActivity {
    vpe: VPE,
    _closure: env::Closure,
}

impl ClosureActivity {
    /// Creates a new `ClosureActivity` for the given VPE and closure.
    pub fn new(vpe: VPE, closure: env::Closure) -> ClosureActivity {
        ClosureActivity {
            vpe,
            _closure: closure,
        }
    }
}

impl Activity for ClosureActivity {
    fn vpe(&self) -> &VPE {
        &self.vpe
    }

    fn vpe_mut(&mut self) -> &mut VPE {
        &mut self.vpe
    }
}

impl Drop for ClosureActivity {
    fn drop(&mut self) {
        self.stop().ok();
    }
}

/// The activity for [`VPE::exec`].
pub struct ExecActivity {
    vpe: VPE,
    _file: BufReader<FileRef>,
}

impl ExecActivity {
    /// Creates a new `ExecActivity` for the given VPE and executable.
    pub fn new(vpe: VPE, file: BufReader<FileRef>) -> ExecActivity {
        ExecActivity { vpe, _file: file }
    }
}

impl Activity for ExecActivity {
    fn vpe(&self) -> &VPE {
        &self.vpe
    }

    fn vpe_mut(&mut self) -> &mut VPE {
        &mut self.vpe
    }
}

impl Drop for ExecActivity {
    fn drop(&mut self) {
        self.stop().ok();
    }
}
