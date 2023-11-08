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

use core::fmt;

use crate::cap::{CapFlags, Capability, SelSpace, Selector};
use crate::errors::Error;
use crate::server::SessId;
use crate::syscalls;

/// Represents a session at the server-side.
pub struct ServerSession {
    cap: Capability,
    creator: usize,
    id: SessId,
}

impl ServerSession {
    /// Creates a new session for server `srv` and given creator using the given id. `auto_close`
    /// specifies whether the CLOSE message should be sent to the server as soon as all derived
    /// session capabilities have been revoked.
    pub fn new(srv: Selector, creator: usize, id: SessId, auto_close: bool) -> Result<Self, Error> {
        let sel = SelSpace::get().alloc_sel();
        Self::new_with_sel(srv, sel, creator, id, auto_close)
    }

    /// Creates a new session for server `srv` and given creator at selector `sel` using the given
    /// id. `auto_close` specifies whether the CLOSE message should be sent to the server as soon
    /// as all derived session capabilities have been revoked.
    pub fn new_with_sel(
        srv: Selector,
        sel: Selector,
        creator: usize,
        id: SessId,
        auto_close: bool,
    ) -> Result<Self, Error> {
        syscalls::create_sess(sel, srv, creator, id as u64, auto_close)?;
        Ok(ServerSession {
            creator,
            cap: Capability::new(sel, CapFlags::empty()),
            id,
        })
    }

    /// Returns the session's capability selector.
    pub fn sel(&self) -> Selector {
        self.cap.sel()
    }

    /// Returns the id of the creator of the session.
    pub fn creator(&self) -> usize {
        self.creator
    }

    /// Returns the id of the session
    pub fn id(&self) -> SessId {
        self.id
    }
}

impl fmt::Debug for ServerSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "ServerSession[sel: {}]", self.sel())
    }
}
