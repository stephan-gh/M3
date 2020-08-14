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

//! The different types that are used to hold the current activity running on a VPE.

use crate::env;
use crate::errors::Error;
use crate::kif;
use crate::pes::VPE;
use crate::syscalls;
use crate::vfs::{BufReader, FileRef};

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

/// The activity for [`VPE::start`].
pub struct DeviceActivity {
    vpe: VPE,
}

impl DeviceActivity {
    /// Creates a new `DeviceActivity` for the given VPE.
    pub fn new(vpe: VPE) -> Self {
        Self { vpe }
    }
}

impl Activity for DeviceActivity {
    fn vpe(&self) -> &VPE {
        &self.vpe
    }

    fn vpe_mut(&mut self) -> &mut VPE {
        &mut self.vpe
    }
}

impl Drop for DeviceActivity {
    fn drop(&mut self) {
        self.stop().ok();
    }
}

/// The activity for [`VPE::run`].
pub struct ClosureActivity {
    vpe: VPE,
    _closure: env::Closure,
}

impl ClosureActivity {
    /// Creates a new `ClosureActivity` for the given VPE and closure.
    pub fn new(vpe: VPE, closure: env::Closure) -> Self {
        Self {
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
    pub fn new(vpe: VPE, file: BufReader<FileRef>) -> Self {
        Self { vpe, _file: file }
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
