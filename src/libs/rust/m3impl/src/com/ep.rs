/*
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

use crate::cap::{CapFlags, Capability, SelSpace, Selector};
use crate::errors::Error;
use crate::kif;
use crate::syscalls;
use crate::tcu::{EpId, TOTAL_EPS};

/// Represents a TCU endpoint that can be used for communication
///
/// Endpoints are allocated by the [`EpMng`](`crate::com::EpMng`), which in turn is typically called
/// automatically by the corresponding gate (e.g., [`SendGate`](`crate::com::SendGate`)) that wants
/// to use it. Therefore, this type is typically not used by applications. However, if an endpoint
/// is obtained from another application, an EP object can be constructed via [`EP::new_bind`] and
/// passed to a gate, for example.
#[derive(Debug)]
pub struct EP {
    cap: Capability,
    ep: EpId,
    replies: u32,
    std: bool,
}

/// The arguments for [`EP`] creations.
pub struct EPArgs {
    epid: EpId,
    act: Selector,
    replies: u32,
}

impl Default for EPArgs {
    /// Creates a new `EPArgs` with default arguments (any EP and no reply slots)
    fn default() -> Self {
        Self {
            epid: TOTAL_EPS,
            act: kif::SEL_ACT,
            replies: 0,
        }
    }
}

impl EPArgs {
    /// Sets the endpoint id to `epid`.
    pub fn epid(mut self, epid: EpId) -> Self {
        self.epid = epid;
        self
    }

    /// Sets the activity to allocate the EP for.
    pub fn activity(mut self, act: Selector) -> Self {
        self.act = act;
        self
    }

    /// Sets the number of reply slots to `slots`.
    pub fn replies(mut self, slots: u32) -> Self {
        self.replies = slots;
        self
    }
}

impl EP {
    const fn create(sel: Selector, ep: EpId, replies: u32, flags: CapFlags, std: bool) -> Self {
        EP {
            cap: Capability::new(sel, flags),
            ep,
            replies,
            std,
        }
    }

    /// Allocates a new endpoint.
    pub(crate) fn new() -> Result<Self, Error> {
        Self::new_with(EPArgs::default())
    }

    /// Allocates a new endpoint with custom arguments
    pub(crate) fn new_with(args: EPArgs) -> Result<Self, Error> {
        let (sel, id) = Self::alloc_cap(args.epid, args.act, args.replies)?;
        Ok(Self::create(
            sel,
            id,
            args.replies,
            CapFlags::empty(),
            false,
        ))
    }

    /// Binds the given selector to a new EP object
    pub fn new_bind(ep: EpId, sel: Selector) -> Self {
        Self::create(sel, ep, 0, CapFlags::KEEP_CAP, false)
    }

    pub(crate) const fn new_def_bind(ep: EpId) -> Self {
        Self::create(kif::INVALID_SEL, ep, 0, CapFlags::KEEP_CAP, true)
    }

    pub(crate) fn destructing_move(&mut self) -> Self {
        let (ep, flags) = (self.ep, self.cap.flags());
        self.cap.set_flags(CapFlags::KEEP_CAP);
        self.ep = TOTAL_EPS;
        Self {
            cap: Capability::new(self.sel(), flags),
            ep,
            replies: self.replies,
            std: self.std,
        }
    }

    /// Returns the endpoint id
    pub fn id(&self) -> EpId {
        self.ep
    }

    /// Returns the endpoint selector
    pub fn sel(&self) -> Selector {
        self.cap.sel()
    }

    /// Returns the number of reply slots
    pub fn replies(&self) -> u32 {
        self.replies
    }

    /// Returns if the EP is a standard EP
    pub fn is_standard(&self) -> bool {
        self.std
    }

    /// Configures this endpoint for the given gate for a different activity. Note that this call
    /// deliberately bypasses the gate object.
    pub fn configure(&self, gate: Selector) -> Result<(), Error> {
        syscalls::activate(self.sel(), gate, kif::INVALID_SEL, 0)
    }

    /// Invalidates this endpoint
    pub fn invalidate(&self) -> Result<(), Error> {
        syscalls::activate(self.sel(), kif::INVALID_SEL, kif::INVALID_SEL, 0)
    }

    fn alloc_cap(epid: EpId, act: Selector, replies: u32) -> Result<(Selector, EpId), Error> {
        let sel = SelSpace::get().alloc_sel();
        let id = syscalls::alloc_ep(sel, act, epid, replies)?;
        Ok((sel, id))
    }
}
