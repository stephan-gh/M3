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

//! The different types that are used to hold the current activity running on a activity.

use crate::errors::Error;
use crate::kif;
use crate::syscalls;
use crate::tiles::Activity;
use crate::vfs::{BufReader, FileRef};

/// Represents an activity that is run on a [`Activity`].
pub trait RunningActivity {
    /// Returns a reference to the activity.
    fn activity(&self) -> &Activity;
    /// Returns a mutable reference to the activity.
    fn activity_mut(&mut self) -> &mut Activity;

    /// Starts the activity.
    fn start(&self) -> Result<(), Error> {
        syscalls::activity_ctrl(self.activity().sel(), kif::syscalls::ActivityOp::START, 0)
            .map(|_| ())
    }

    /// Stops the activity.
    fn stop(&self) -> Result<(), Error> {
        syscalls::activity_ctrl(self.activity().sel(), kif::syscalls::ActivityOp::STOP, 0)
            .map(|_| ())
    }

    /// Waits until the activity exits and returns the error code.
    fn wait(&self) -> Result<i32, Error> {
        syscalls::activity_wait(&[self.activity().sel()], 0).map(|r| r.1)
    }

    /// Starts an asynchronous wait for the activity, using the given event for the upcall.
    fn wait_async(&self, event: u64) -> Result<i32, Error> {
        syscalls::activity_wait(&[self.activity().sel()], event).map(|r| r.1)
    }
}

/// The activity for [`Activity::start`].
pub struct RunningDeviceActivity {
    act: Activity,
}

impl RunningDeviceActivity {
    /// Creates a new `DeviceActivity` for the given activity.
    pub fn new(act: Activity) -> Self {
        Self { act }
    }
}

impl RunningActivity for RunningDeviceActivity {
    fn activity(&self) -> &Activity {
        &self.act
    }

    fn activity_mut(&mut self) -> &mut Activity {
        &mut self.act
    }
}

impl Drop for RunningDeviceActivity {
    fn drop(&mut self) {
        self.stop().ok();
    }
}

/// The activity for [`Activity::run`] and [`Activity::exec`].
pub struct RunningProgramActivity {
    act: Activity,
    _file: BufReader<FileRef>,
}

impl RunningProgramActivity {
    /// Creates a new `ExecActivity` for the given activity and executable.
    pub fn new(act: Activity, file: BufReader<FileRef>) -> Self {
        Self { act, _file: file }
    }
}

impl RunningActivity for RunningProgramActivity {
    fn activity(&self) -> &Activity {
        &self.act
    }

    fn activity_mut(&mut self) -> &mut Activity {
        &mut self.act
    }
}

impl Drop for RunningProgramActivity {
    fn drop(&mut self) {
        self.stop().ok();
    }
}
