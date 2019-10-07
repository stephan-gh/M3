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
use syscalls;
use vpe::VPE;

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
    const fn create(sel: Selector, ep: Option<EpId>, free: bool) -> Self {
        EP {
            cap: Capability::new(sel, CapFlags::KEEP_CAP),
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
            // TODO actually: VPE.runs_on_pemux()
            let (sel, id) = Self::alloc_cap(vpe)?;
            return Ok(Self::create(sel, Some(id), true));
        }

        let id = vpe.epmng().alloc_ep()?;
        Ok(Self::create(Self::sel_of_vpe(vpe, id), Some(id), true))
    }

    pub(crate) const fn new_def_bind(ep: EpId) -> Self {
        Self::create(Self::sel_of(ep), Some(ep), false)
    }

    /// Creates a new endpoint object that is bound to the given endpoint id.
    pub fn new_bind(ep: Option<EpId>) -> Self {
        let sel = match ep {
            Some(ep) => Self::sel_of(ep),
            None => kif::INVALID_SEL,
        };
        Self::create(sel, ep, false)
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
        vpe.sel() + Self::sel_of(ep)
    }

    fn alloc_cap(vpe: &VPE) -> Result<(Selector, EpId), Error> {
        let sel = VPE::cur().alloc_sel();
        let resmng = VPE::cur().resmng();
        let id = if resmng.sel() == kif::INVALID_SEL {
            syscalls::alloc_ep(sel, vpe.sel())
        }
        else {
            resmng.alloc_ep(sel, vpe.sel())
        }?;
        Ok((sel, id))
    }
}

impl Drop for EP {
    fn drop(&mut self) {
        if self.free {
            assert!(self.valid());
            if env::get().shared() {
                VPE::cur().resmng().free_ep(self.sel()).ok();
            }
            else {
                VPE::cur().epmng().free_ep(self.ep.get().unwrap());
            }
        }
    }
}
