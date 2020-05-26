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

use cap::Selector;
use col::Vec;
use com::{RecvGate, SGateArgs, SendGate};
use errors::{Code, Error};
use pes::VPE;
use session::ServerSession;
use tcu::Label;

pub(crate) const MAX_CREATORS: usize = 3;

/// Used as session identifier
pub type SessId = usize;

struct Creator {
    // the creator's `SendGate` to communicate with us
    _sgate: SendGate,
    // the remaining number of sessions that can be created
    sessions: usize,
    // keep a bitmask of sessions belonging to this creator
    sids: u64,
}

/// A container for sessions.
pub struct SessionContainer<S> {
    capacity: usize,
    con: Vec<Option<S>>,
    creators: Vec<Creator>,
    used: u64,
}

impl<S> SessionContainer<S> {
    /// Creates a new `SessionContainer` with a at most `capacity` sessions.
    pub fn new(capacity: usize) -> Self {
        let mut con = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            con.push(None);
        }

        SessionContainer {
            capacity,
            con,
            creators: Vec::new(),
            used: 0,
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
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

    /// Adds a new creator with given amount of sessions
    pub fn add_creator(
        &mut self,
        rgate: &RecvGate,
        sessions: usize,
    ) -> Result<(usize, Selector), Error> {
        let nid = self.creators.len();
        let _sgate = SendGate::new_with(SGateArgs::new(rgate).credits(1).label(nid as Label))?;
        let sgate_sel = _sgate.sel();
        let ncrt = Creator {
            _sgate,
            sessions,
            sids: 0,
        };
        self.creators.push(ncrt);
        Ok((nid, sgate_sel))
    }

    /// Derives a new creator from `crt` with the given amount of sessions
    pub fn derive_creator(
        &mut self,
        rgate: &RecvGate,
        crt: usize,
        sessions: usize,
    ) -> Result<(usize, Selector), Error> {
        if sessions > self.creators[crt].sessions || self.creators.len() == MAX_CREATORS {
            return Err(Error::new(Code::NoSpace));
        }

        let (nid, sel) = self.add_creator(rgate, sessions)?;
        self.creators[crt].sessions -= sessions;
        Ok((nid, sel))
    }

    /// Returns the session quota for the given creator
    pub fn sess_quota(&self, idx: usize) -> usize {
        self.creators[idx].sessions
    }

    /// Returns true if the given creator owns the given session
    pub fn creator_owns(&self, idx: usize, sid: SessId) -> bool {
        (self.creators[idx].sids & (1 << sid)) != 0
    }

    /// Returns a reference to the session with given id
    pub fn get(&self, sid: SessId) -> Option<&S> {
        self.con[sid].as_ref()
    }

    /// Returns a mutable reference to the session with given id
    pub fn get_mut(&mut self, sid: SessId) -> Option<&mut S> {
        self.con[sid].as_mut()
    }

    /// Returns mutable references to the sessions with ids `sid1` and `sid2`
    pub fn get_two_mut(&mut self, sid1: SessId, sid2: SessId) -> (Option<&mut S>, Option<&mut S>) {
        assert!(sid1 != sid2);
        assert!(sid1 < self.con.len());
        assert!(sid2 < self.con.len());

        // safety: we have a mutable reference to self, so we can hand out two mutable references
        // to two members during that time.
        unsafe {
            let ptr = self.con.as_mut_slice().as_mut_ptr();
            let s1 = (*ptr.add(sid1)).as_mut();
            let s2 = (*ptr.add(sid2)).as_mut();
            (s1, s2)
        }
    }

    /// Returns true if the given creator can add another session
    pub fn can_add(&self, crt: usize) -> bool {
        crt < self.creators.len() && self.creators[crt].sessions > 0
    }

    /// Adds a new session with given id, assuming that the id is not in use.
    pub fn add(&mut self, crt: usize, sid: SessId, sess: S) -> Result<(), Error> {
        // check and reduce session quota
        if !self.can_add(crt) {
            return Err(Error::new(Code::NoSpace));
        }
        self.creators[crt].sids |= 1 << sid;
        self.creators[crt].sessions -= 1;

        assert!(self.used & (1 << sid) == 0);
        self.con[sid] = Some(sess);
        self.used |= 1 << sid;
        Ok(())
    }

    /// Adds a new session with the next available id and a selector allocated from [`VPE::cur()`].
    /// The session is created by `create_sess`, which takes a new [`ServerSession`] object for the
    /// service denoted by `srv_sel`. The parameter `auto_close` is passed to [`ServerSession`] on
    /// creation.
    pub fn add_next<F>(
        &mut self,
        crt: usize,
        srv_sel: Selector,
        auto_close: bool,
        create_sess: F,
    ) -> Result<(Selector, SessId), Error>
    where
        F: Fn(ServerSession) -> Result<S, Error>,
    {
        let sid = self.next_id()?;
        let sel = VPE::cur().alloc_sel();
        let sess = create_sess(ServerSession::new_with_sel(
            srv_sel, sel, crt, sid as u64, auto_close,
        )?)?;
        self.add(crt, sid, sess).map(|_| (sel, sid))
    }

    /// Removes the session with given id, assuming that the session exists.
    pub fn remove(&mut self, crt: usize, sid: SessId) {
        // increase session quota
        assert!(crt < self.creators.len());
        self.creators[crt].sids &= !(1 << sid);
        self.creators[crt].sessions += 1;

        assert!(self.used & (1 << sid) != 0);
        self.con[sid] = None;
        self.used &= !(1 << sid);
    }
}
