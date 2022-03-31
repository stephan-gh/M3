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

use crate::cap::{CapFlags, Capability, Selector};
use crate::errors::Error;
use crate::syscalls;
use crate::tiles::Activity;

/// Represents a session at the server-side.
pub struct ServerSession {
    cap: Capability,
    ident: u64,
}

impl ServerSession {
    /// Creates a new session for server `srv` and given creator using the given ident. `auto_close`
    /// specifies whether the CLOSE message should be sent to the server as soon as all derived
    /// session capabilities have been revoked.
    pub fn new(srv: Selector, creator: usize, ident: u64, auto_close: bool) -> Result<Self, Error> {
        let sel = Activity::own().alloc_sel();
        Self::new_with_sel(srv, sel, creator, ident, auto_close)
    }

    /// Creates a new session for server `srv` and given creator at selector `sel` using the given
    /// ident. `auto_close` specifies whether the CLOSE message should be sent to the server as soon
    /// as all derived session capabilities have been revoked.
    pub fn new_with_sel(
        srv: Selector,
        sel: Selector,
        creator: usize,
        ident: u64,
        auto_close: bool,
    ) -> Result<Self, Error> {
        syscalls::create_sess(sel, srv, creator, ident, auto_close)?;
        Ok(ServerSession {
            cap: Capability::new(sel, CapFlags::empty()),
            ident,
        })
    }

    /// Returns the session's capability selector.
    pub fn sel(&self) -> Selector {
        self.cap.sel()
    }

    /// Returns the ident of the session
    pub fn ident(&self) -> u64 {
        self.ident
    }
}

impl fmt::Debug for ServerSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "ServerSession[sel: {}]", self.sel())
    }
}
