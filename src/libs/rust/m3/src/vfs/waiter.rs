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
    files: Vec<(Fd, FileEvent)>,
}

impl FileWaiter {
    /// Adds the given file descriptor with given events to the set of files that this `FileWaiter`
    /// waits for.
    ///
    /// This method assumes that the file descriptor has not been given to this waiter yet.
    pub fn add(&mut self, fd: Fd, events: FileEvent) {
        self.files.push((fd, events));
    }

    /// Adds or sets the given events for the given file descriptor. If the file descriptor already
    /// exists, the events are updated. Otherwise, a new entry is created.
    pub fn set(&mut self, fd: Fd, events: FileEvent) {
        if let Some((_, ref mut cur_events)) = self.files.iter_mut().find(|(id, _)| *id == fd) {
            *cur_events = events;
        }
        else {
            self.add(fd, events);
        }
    }

    /// Removes the given file descriptor from the set of files that this `FileWaiter` waits for.
    pub fn remove(&mut self, fd: Fd) {
        self.files.retain(|(id, _)| *id != fd);
    }

    /// Waits until any file has received any of the desired events.
    ///
    /// Note also that this function uses
    /// [`Activity::own().sleep`](crate::tiles::OwnActivity::sleep) if no read/write on any file is
    /// possible, which suspends the core until the next TCU message arrives. Thus, calling this
    /// function can only be done if all work is done.
    pub fn wait(&self) {
        loop {
            if self.tick_files() {
                break;
            }

            // ignore errors
            Activity::own().sleep().ok();
        }
    }

    /// Waits until any file has received any of the desired events or the given timeout in
    /// nanoseconds is reached.
    ///
    /// Note also that this function uses
    /// [`Activity::own().sleep`](crate::tiles::OwnActivity::sleep) if no read/write on any file is
    /// possible, which suspends the core until the next TCU message arrives. Thus, calling this
    /// function can only be done if all work is done.
    pub fn wait_for(&self, timeout: TimeDuration) {
        let end = TimeInstant::now() + timeout;
        loop {
            let now = TimeInstant::now();
            let duration = end.checked_duration_since(now);
            if duration.is_none() || self.tick_files() {
                break;
            }

            // ignore errors
            Activity::own().sleep_for(duration.unwrap()).ok();
        }
    }

    /// Sleep for the given duration, respecting events that may arrive for files.
    ///
    /// Note that this function uses [`Activity::own().sleep`](crate::tiles::OwnActivity::sleep) if
    /// no read/write on any file is possible, which suspends the core until the next TCU message
    /// arrives. Thus, calling this function can only be done if all work is done.
    pub fn sleep_for(&self, duration: TimeDuration) {
        let end = TimeInstant::now() + duration;
        loop {
            self.tick_files();

            let now = TimeInstant::now();
            match end.checked_duration_since(now) {
                // ignore errors
                Some(d) => Activity::own().sleep_for(d).ok(),
                None => break,
            };
        }
    }

    fn tick_files(&self) -> bool {
        let mut found = false;
        for (fd, events) in &self.files {
            let files = Activity::own().files();
            if let Some(mut file) = files.get(*fd) {
                // accessing the file requires that we don't hold a references to the filetable
                drop(files);
                if file.check_events(*events) {
                    found = true;
                }
            }
        }
        found
    }
}
