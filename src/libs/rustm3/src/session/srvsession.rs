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

use cap::{CapFlags, Capability, Selector};
use core::fmt;
use errors::Error;
use syscalls;
use vpe;

/// Represents a session at the server-side.
pub struct ServerSession {
    cap: Capability,
}

impl ServerSession {
    /// Creates a new session for server `srv` using the given ident.
    pub fn new(srv: Selector, ident: u64) -> Result<Self, Error> {
        let sel = vpe::VPE::cur().alloc_sel();
        Self::new_with_sel(srv, sel, ident)
    }

    /// Creates a new session for server `srv` at selector `sel` using the given ident.
    pub fn new_with_sel(srv: Selector, sel: Selector, ident: u64) -> Result<Self, Error> {
        syscalls::create_sess(sel, srv, ident)?;
        Ok(ServerSession {
            cap: Capability::new(sel, CapFlags::empty()),
        })
    }

    /// Returns the session's capability selector.
    pub fn sel(&self) -> Selector {
        self.cap.sel()
    }
}

impl fmt::Debug for ServerSession {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "ServerSession[sel: {}]", self.sel())
    }
}
