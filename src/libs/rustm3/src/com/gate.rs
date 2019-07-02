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
use cell::Cell;
use com::EpMux;
use core::ops;
use dtu::EpId;
use errors::Error;

/// A gate is one side of a DTU-based communication channel and exists in the variants [`MemGate`],
/// [`SendGate`], and [`RecvGate`].
#[derive(Debug)]
pub struct Gate {
    cap: Capability,
    ep: Cell<Option<EpId>>,
}

impl Gate {
    /// Creates a new gate with given capability selector and flags
    pub fn new(sel: Selector, flags: CapFlags) -> Self {
        Self::new_with_ep(sel, flags, None)
    }

    /// Creates a new gate with given capability selector, flags, and endpoint
    pub const fn new_with_ep(sel: Selector, flags: CapFlags, ep: Option<EpId>) -> Self {
        Gate {
            cap: Capability::new(sel, flags),
            ep: Cell::new(ep),
        }
    }

    /// Returns the capability selector
    pub fn sel(&self) -> Selector {
        self.cap.sel()
    }

    /// Returns the flags that determine whether the capability will be revoked on destruction
    pub fn flags(&self) -> CapFlags {
        self.cap.flags()
    }
    pub(crate) fn set_flags(&mut self, flags: CapFlags) {
        self.cap.set_flags(flags);
    }

    /// Returns the endpoint. If the gate is not activated, it returns `None`.
    pub fn ep(&self) -> Option<EpId> {
        self.ep.get()
    }

    pub(crate) fn set_ep(&self, ep: EpId) {
        self.ep.set(Some(ep));
        EpMux::get().set_owned(ep, self.sel());
    }
    pub(crate) fn unset_ep(&self) {
        if let Some(ep) = self.ep() {
            EpMux::get().unset_owned(ep);
        }
        self.ep.set(None);
    }

    /// Activates the gate, if not already done, potentially involving endpoint multiplexing.
    /// Returns the chosen endpoint number.
    pub fn activate(&self) -> Result<EpId, Error> {
        // the invariants here are:
        // 1. if ep is Some, ep_owned_by determines whether we currently own that EP.
        //    (it might have been reused for something else behind our back)
        // 2. if ep is None, we don't have an EP yet and need to get one via switch_to
        // the first implies that if we configure EPs otherwise for a gate (for example, in
        // genericfile), we have to mark it owned in EpMux. That's why we set/unset it owned above.
        match self.ep() {
            Some(ep) if EpMux::get().ep_owned_by(ep, self.sel()) => Ok(ep),
            _                                                    => EpMux::get().switch_to(self),
        }
    }

    /// Switches the underlying capability selector to `sel`. If the gate is currently activated,
    /// it will be reactivated with the given capability selector.
    pub fn rebind(&mut self, sel: Selector) -> Result<(), Error> {
        EpMux::get().switch_cap(self, sel)?;
        self.cap.rebind(sel);
        Ok(())
    }
}

impl ops::Drop for Gate {
    fn drop(&mut self) {
        EpMux::get().remove(self);
    }
}
