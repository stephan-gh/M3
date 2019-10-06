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
use com::EP;
use core::ops;
use core::mem;
use dtu::EpId;
use dtuif;
use errors::Error;
use vpe::VPE;

/// A gate is one side of a DTU-based communication channel and exists in the variants [`MemGate`],
/// [`SendGate`], and [`RecvGate`].
#[derive(Debug)]
pub struct Gate {
    cap: Capability,
    ep: EP,
}

impl Gate {
    /// Creates a new gate with given capability selector and flags
    pub fn new(sel: Selector, flags: CapFlags) -> Self {
        Gate {
            cap: Capability::new(sel, flags),
            ep: EP::new_bind(None),
        }
    }

    /// Creates a new gate with given capability selector, flags, and endpoint
    pub const fn new_with_ep(sel: Selector, flags: CapFlags, ep: EpId) -> Self {
        Gate {
            cap: Capability::new(sel, flags),
            ep: EP::new_def_bind(ep),
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
    pub(crate) fn ep(&self) -> Option<EpId> {
        self.ep.id()
    }

    pub(crate) fn put_ep(&mut self, mut ep: EP) -> Result<(), Error> {
        ep.assign(self.sel())?;
        if let Some(ep) = ep.id() {
            VPE::cur().epmng().set_owned(ep, self.sel());
        }
        self.ep = ep;
        Ok(())
    }
    pub(crate) fn take_ep(&mut self) -> EP {
        if let Some(ep) = self.ep.id() {
            VPE::cur().epmng().set_unowned(ep);
        }
        mem::replace(&mut self.ep, EP::new_bind(None))
    }

    pub(crate) fn set_epid(&self, ep: EpId) {
        self.ep.set_id(Some(ep));
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
        let epmng = VPE::cur().epmng();
        match self.ep() {
            Some(ep) if epmng.ep_owned_by(ep, self.sel()) => Ok(ep),
            _ => epmng.switch_to(self),
        }
    }
}

impl ops::Drop for Gate {
    fn drop(&mut self) {
        dtuif::DTUIf::remove_gate(self, self.cap.flags().contains(CapFlags::KEEP_CAP)).ok();
    }
}
