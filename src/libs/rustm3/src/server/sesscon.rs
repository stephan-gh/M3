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

use col::Vec;
use errors::{Code, Error};

/// Used as session identifier
pub type SessId = usize;

/// A container for sessions.
pub struct SessionContainer<S> {
    con: Vec<Option<S>>,
    used: u64,
}

impl<S> SessionContainer<S> {
    /// Creates a new `SessionContainer` with a at most `max` sessions.
    pub fn new(max: usize) -> Self {
        let mut con = Vec::with_capacity(max);
        for _ in 0..max {
            con.push(None);
        }

        SessionContainer {
            con: con,
            used: 0,
        }
    }

    /// Returns the id that will be used for the next session
    pub fn next_id(&self) -> Result<SessId, Error> {
        for i in 0..self.con.capacity() {
            if self.used & (1 << i) == 0 {
                return Ok(i);
            }
        }
        Err(Error::new(Code::NoSpace))
    }

    /// Returns a reference to the session with given id
    pub fn get(&self, sid: SessId) -> Option<&S> {
        self.con[sid].as_ref()
    }
    /// Returns a mutable reference to the session with given id
    pub fn get_mut(&mut self, sid: SessId) -> Option<&mut S> {
        self.con[sid].as_mut()
    }

    /// Adds a new session with given id, assuming that the id is not in use.
    pub fn add(&mut self, sid: SessId, sess: S) {
        assert!(self.used & (1 << sid) == 0);
        self.con[sid] = Some(sess);
        self.used |= 1 << sid;
    }

    /// Removes the session with given id, assuming that the session exists.
    pub fn remove(&mut self, sid: SessId) {
        assert!(self.used & (1 << sid) != 0);
        self.con[sid] = None;
        self.used &= !(1 << sid);
    }
}
