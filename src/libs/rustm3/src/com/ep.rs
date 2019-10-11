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

use arch::env;
use cap::{CapFlags, Capability, Selector};
use cell::Cell;
use dtu::{EpId, FIRST_FREE_EP};
use dtuif;
use errors::Error;
use kif;
use pes::VPE;
use syscalls;

/// Represents a DTU endpoint that can be used for communication. This class only serves the purpose
/// to allocate a EP capability and revoke it on destruction. In the meantime, the EP capability can
/// be delegated to someone else.
#[derive(Debug)]
pub struct EP {
    cap: Capability,
    ep: Cell<Option<EpId>>,
    free: bool,
}

impl EP {
    const fn create(sel: Selector, ep: Option<EpId>, free: bool, flags: CapFlags) -> Self {
        EP {
            cap: Capability::new(sel, flags),
            ep: Cell::new(ep),
            free,
        }
    }

    /// Allocates a new endpoint.
    pub fn new() -> Result<Self, Error> {
        Self::new_for(VPE::cur())
    }

    /// Allocates a new endpoint for given VPE.
    pub fn new_for(vpe: &mut VPE) -> Result<Self, Error> {
        if env::get().shared() {
            let (sel, id) = Self::alloc_cap(vpe)?;
            return Ok(Self::create(sel, Some(id), false, CapFlags::empty()));
        }

        let id = vpe.epmng().alloc_ep()?;
        Ok(Self::create(
            Self::sel_of_vpe(vpe, id),
            Some(id),
            true,
            CapFlags::KEEP_CAP,
        ))
    }

    pub(crate) const fn new_def_bind(ep: EpId) -> Self {
        Self::create(Self::sel_of(ep), Some(ep), false, CapFlags::KEEP_CAP)
    }

    /// Creates a new endpoint object that is bound to the given endpoint id.
    pub fn new_bind(ep: Option<EpId>) -> Self {
        let sel = match ep {
            Some(ep) => Self::sel_of(ep),
            None => kif::INVALID_SEL,
        };
        Self::create(sel, ep, false, CapFlags::KEEP_CAP)
    }

    /// Returns true if the endpoint is valid, i.e., has a selector and endpoint id
    pub fn valid(&self) -> bool {
        self.ep.get().is_some()
    }

    /// Returns the endpoint id
    pub fn id(&self) -> Option<EpId> {
        self.ep.get()
    }

    /// Returns the endpoint selector
    pub fn sel(&self) -> Selector {
        self.cap.sel()
    }

    pub(crate) fn assign(&mut self, gate: Selector) -> Result<(), Error> {
        dtuif::DTUIf::switch_gate(self, gate)
    }

    pub(crate) fn set_id(&self, ep: Option<EpId>) {
        self.ep.set(ep);
    }

    pub(crate) const fn sel_of(ep: EpId) -> Selector {
        kif::FIRST_EP_SEL + ep as Selector - FIRST_FREE_EP as Selector
    }

    pub(crate) fn sel_of_vpe(vpe: &VPE, ep: EpId) -> Selector {
        const_assert!(kif::SEL_PE == 0);
        vpe.pe().sel() + Self::sel_of(ep)
    }

    fn alloc_cap(vpe: &VPE) -> Result<(Selector, EpId), Error> {
        let sel = VPE::cur().alloc_sel();
        let id = syscalls::alloc_ep(sel, vpe.sel(), vpe.pe().sel())?;
        Ok((sel, id))
    }
}

impl Drop for EP {
    fn drop(&mut self) {
        if self.free {
            assert!(self.valid());
            VPE::cur().epmng().free_ep(self.ep.get().unwrap());
        }
    }
}
