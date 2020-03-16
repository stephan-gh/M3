/*
 * Copyright (C) 2016-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

use arch;
use cap::{CapFlags, Capability, Selector};
use tcu::{EpId, EP_COUNT, STD_EPS_COUNT};
use errors::Error;
use kif;
use pes::VPE;
use syscalls;

/// Represents a TCU endpoint that can be used for communication. This class only serves the purpose
/// to allocate a EP capability and revoke it on destruction. In the meantime, the EP capability can
/// be delegated to someone else.
#[derive(Debug)]
pub struct EP {
    cap: Capability,
    ep: EpId,
    replies: u32,
}

/// The arguments for [`EP`] creations.
pub struct EPArgs {
    epid: EpId,
    replies: u32,
}

impl EPArgs {
    /// Creates a new `EPArgs` with default arguments (any EP and no reply slots)
    pub fn new() -> Self {
        EPArgs {
            epid: EP_COUNT,
            replies: 0,
        }
    }

    /// Sets the endpoint id to `epid`.
    pub fn epid(mut self, epid: EpId) -> Self {
        self.epid = epid;
        self
    }

    /// Sets the number of reply slots to `slots`.
    pub fn replies(mut self, slots: u32) -> Self {
        self.replies = slots;
        self
    }
}

impl EP {
    const fn create(sel: Selector, ep: EpId, replies: u32, flags: CapFlags) -> Self {
        EP {
            cap: Capability::new(sel, flags),
            ep,
            replies,
        }
    }

    /// Allocates a new endpoint.
    pub(crate) fn new() -> Result<Self, Error> {
        Self::new_with(EPArgs::new())
    }

    /// Allocates a new endpoint with custom arguments
    pub(crate) fn new_with(args: EPArgs) -> Result<Self, Error> {
        let (sel, id) = Self::alloc_cap(args.epid, args.replies)?;
        return Ok(Self::create(sel, id, args.replies, CapFlags::empty()));
    }

    pub(crate) const fn new_def_bind(ep: EpId) -> Self {
        Self::create(kif::INVALID_SEL, ep, 0, CapFlags::KEEP_CAP)
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
        let eps_start = arch::env::get().first_std_ep();
        self.id() >= eps_start && self.id() < eps_start + STD_EPS_COUNT
    }

    fn alloc_cap(epid: EpId, replies: u32) -> Result<(Selector, EpId), Error> {
        let sel = VPE::cur().alloc_sel();
        let id = syscalls::alloc_ep(sel, VPE::cur().sel(), epid, replies)?;
        Ok((sel, id))
    }
}
