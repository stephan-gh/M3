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

use core::ops;

use crate::cap::{CapFlags, Capability, Selector};
use crate::cell::{Cell, Ref, RefCell};
use crate::com::{EpMng, EP};
use crate::errors::Error;
use crate::kif;
use crate::mem::GlobOff;
use crate::syscalls;
use crate::tcu::EpId;
use crate::tiles::Activity;

/// A gate is one side of a TCU-based communication channel and exists in the variants
/// [`MemGate`](`crate::com::MemGate`), [`SendGate`](`crate::com::SendGate`), and
/// [`RecvGate`](`crate::com::RecvGate`).
pub struct Gate {
    cap: Capability,
    // keep the endpoint id separately in a Cell for a cheaper access. most of the time, we only
    // need the EP id, so that we can avoid borrowing the RefCell.
    epid: Cell<Option<EpId>>,
    ep: RefCell<Option<EP>>,
}

impl Gate {
    /// Creates a new gate with given capability selector and flags
    pub fn new(sel: Selector, flags: CapFlags) -> Self {
        Gate {
            cap: Capability::new(sel, flags),
            epid: Cell::new(None),
            ep: RefCell::new(None),
        }
    }

    /// Creates a new gate with given capability selector, flags, and endpoint
    pub const fn new_with_ep(sel: Selector, flags: CapFlags, epid: EpId) -> Self {
        Gate {
            cap: Capability::new(sel, flags),
            epid: Cell::new(Some(epid)),
            ep: RefCell::new(Some(EP::new_def_bind(epid))),
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

    /// Sets the flags to given ones.
    pub(crate) fn set_flags(&mut self, flags: CapFlags) {
        self.cap.set_flags(flags);
    }

    /// Returns the endpoint id. If the gate is not activated, it returns [`None`].
    pub(crate) fn epid(&self) -> Option<EpId> {
        self.epid.get()
    }

    /// Returns the endpoint. If the gate is not activated, it returns [`None`].
    pub(crate) fn ep(&self) -> Option<Ref<'_, EP>> {
        if self.epid.get().is_some() {
            Some(Ref::map(self.ep.borrow(), |ep| ep.as_ref().unwrap()))
        }
        else {
            None
        }
    }

    /// Sets or unsets the endpoint.
    pub(crate) fn set_ep(&self, ep: Option<EP>) {
        self.epid.replace(ep.as_ref().map(|obj| obj.id()));
        self.ep.replace(ep);
    }

    /// Activates the gate. Returns the chosen endpoint number.
    pub(crate) fn activate_rgate(
        &self,
        mem: Option<Selector>,
        addr: GlobOff,
        replies: u32,
    ) -> Result<EpId, Error> {
        let ep = EpMng::get().acquire(replies)?;
        syscalls::activate(ep.sel(), self.sel(), mem.unwrap_or(kif::INVALID_SEL), addr)?;
        self.set_ep(Some(ep));
        Ok(self.epid().unwrap())
    }

    /// Activates the gate. Returns the chosen endpoint number.
    #[inline(always)]
    pub(crate) fn activate(&self) -> Result<EpId, Error> {
        if let Some(ep) = self.epid() {
            return Ok(ep);
        }

        self.do_activate()
    }

    fn do_activate(&self) -> Result<EpId, Error> {
        let ep = EpMng::get().activate(self)?;
        self.set_ep(Some(ep));
        Ok(self.epid().unwrap())
    }

    /// Releases the EP that is used by this gate
    pub(crate) fn release(&mut self, force_inval: bool) {
        if let Some(ep) = self.ep.replace(None) {
            EpMng::get().release(
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
