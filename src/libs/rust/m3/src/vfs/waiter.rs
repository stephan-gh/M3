/*
 * Copyright (C) 2022 Nils Asmussen, Barkhausen Institut
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

use crate::col::Vec;
use crate::tiles::Activity;
use crate::time::{TimeDuration, TimeInstant};
use crate::vfs::{Fd, File, FileEvent};

#[derive(Default)]
pub struct FileWaiter {
    files: Vec<Fd>,
}

impl FileWaiter {
    /// Adds the given file descriptor to the set of files that this `FileWaiter` waits for.
    pub fn add(&mut self, fd: Fd) {
        self.files.push(fd);
    }

    /// Removes the given file descriptor from the set of files that this `FileWaiter` waits for.
    pub fn remove(&mut self, fd: Fd) {
        self.files.retain(|id| *id != fd);
    }

    /// Waits until any file has received any of the given events.
    ///
    /// Note also that this function uses [`Activity::sleep`] if no read/write on any file is
    /// possible, which suspends the core until the next TCU message arrives. Thus, calling this
    /// function can only be done if all work is done.
    pub fn wait(&self, events: FileEvent) {
        loop {
            if self.tick_files(events) {
                break;
            }

            // ignore errors
            Activity::own().sleep().ok();
        }
    }

    /// Waits until any file has received any of the given events or the given timeout in
    /// nanoseconds is reached.
    ///
    /// Note also that this function uses [`Activity::sleep`] if no read/write on any file is
    /// possible, which suspends the core until the next TCU message arrives. Thus, calling this
    /// function can only be done if all work is done.
    pub fn wait_for(&self, timeout: TimeDuration, events: FileEvent) {
        let end = TimeInstant::now() + timeout;
        loop {
            let now = TimeInstant::now();
            let duration = end.checked_duration_since(now);
            if duration.is_none() || self.tick_files(events) {
                break;
            }

            // ignore errors
            Activity::own().sleep_for(duration.unwrap()).ok();
        }
    }

    /// Sleep for the given duration, respecting events that may arrive for files.
    ///
    /// Note that this function uses [`Activity::sleep`] if no read/write on any file is possible,
    /// which suspends the core until the next TCU message arrives. Thus, calling this function can
    /// only be done if all work is done.
    pub fn sleep_for(&self, duration: TimeDuration) {
        let end = TimeInstant::now() + duration;
        loop {
            self.tick_files(FileEvent::empty());

            let now = TimeInstant::now();
            match end.checked_duration_since(now) {
                // ignore errors
                Some(d) => Activity::own().sleep_for(d).ok(),
                None => break,
            };
        }
    }

    fn tick_files(&self, events: FileEvent) -> bool {
        let mut found = false;
        for fd in &self.files {
            let files = Activity::own().files();
            if let Some(mut file) = files.get(*fd) {
                // accessing the file requires that we don't hold a references to the filetable
                drop(files);
                if file.check_events(events) {
                    found = true;
                }
            }
        }
        found
    }
}
