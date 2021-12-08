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

use core::ops;

use crate::cap::{CapFlags, Capability, Selector};
use crate::cell::Cell;
use crate::com::{EpMng, EP};
use crate::errors::{Code, Error};
use crate::kif;
use crate::pes::VPE;
use crate::syscalls;
use crate::tcu::EpId;

/// A gate is one side of a TCU-based communication channel and exists in the variants [`MemGate`],
/// [`SendGate`], and [`RecvGate`].
pub struct Gate {
    cap: Capability,
    ep: Cell<Option<EP>>,
}

impl Gate {
    /// Creates a new gate with given capability selector and flags
    pub fn new(sel: Selector, flags: CapFlags) -> Self {
        Gate {
            cap: Capability::new(sel, flags),
            ep: Cell::new(None),
        }
    }

    /// Creates a new gate with given capability selector, flags, and endpoint
    pub const fn new_with_ep(sel: Selector, flags: CapFlags, ep: EpId) -> Self {
        Gate {
            cap: Capability::new(sel, flags),
            ep: Cell::new(Some(EP::new_def_bind(ep))),
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

    /// Returns the endpoint. If the gate is not activated, it returns [`None`].
    pub(crate) fn ep(&self) -> Option<&EP> {
        // why is there no method that gives us a immutable reference to the Cell's inner value?
        unsafe { (*self.ep.as_ptr()).as_ref() }
    }

    /// Returns the endpoint. If the gate is not activated, it returns [`None`].
    pub(crate) fn ep_id(&self) -> Option<EpId> {
        self.ep().map(|ep| ep.id())
    }

    /// Activates the gate. Returns the chosen endpoint number.
    pub(crate) fn activate_rgate(
        &self,
        mem: Option<Selector>,
        addr: usize,
        replies: u32,
    ) -> Result<EpId, Error> {
        let ep = VPE::cur().epmng_mut().acquire(replies)?;
        syscalls::activate(ep.sel(), self.sel(), mem.unwrap_or(kif::INVALID_SEL), addr)?;
        self.ep.replace(Some(ep));
        Ok(self.ep_id().unwrap())
    }

    /// Activates the gate on given EP.
    #[inline(always)]
    pub(crate) fn activate_for(
        &self,
        vpe: Selector,
        epid: EpId,
        replies: u32,
    ) -> Result<(), Error> {
        if self.ep().is_some() {
            return Err(Error::new(Code::Exists));
        }

        let ep = EpMng::acquire_for(vpe, epid, replies)?;
        syscalls::activate(ep.sel(), self.sel(), kif::INVALID_SEL, 0)?;
        self.ep.set(Some(ep));
        Ok(())
    }

    /// Activates the gate. Returns the chosen endpoint number.
    #[inline(always)]
    pub(crate) fn activate(&self) -> Result<&EP, Error> {
        if let Some(ep) = self.ep() {
            return Ok(ep);
        }

        self.do_activate()
    }

    fn do_activate(&self) -> Result<&EP, Error> {
        let ep = VPE::cur().epmng_mut().activate(self)?;
        self.ep.replace(Some(ep));
        Ok(self.ep().unwrap())
    }

    /// Releases the EP that is used by this gate
    pub(crate) fn release(&mut self, force_inval: bool) {
        if let Some(ep) = self.ep.replace(None) {
            VPE::cur().epmng_mut().release(
                ep,
                force_inval || self.cap.flags().contains(CapFlags::KEEP_CAP),
            );
        }
    }
}

impl ops::Drop for Gate {
    fn drop(&mut self) {
        self.release(false);
    }
}
