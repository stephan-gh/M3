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

use core::fmt;

use crate::cap::{CapFlags, Capability, Selector};
use crate::errors::{Code, Error};
use crate::kif::PEDesc;
use crate::pes::VPE;
use crate::quota::Quota;
use crate::rc::Rc;
use crate::syscalls;
use crate::tcu::PEId;

/// Represents a processing element.
pub struct PE {
    cap: Capability,
    id: PEId,
    desc: PEDesc,
    free: bool,
}

impl PE {
    /// Allocates a new PE from the resource manager with given description
    pub fn new(desc: PEDesc) -> Result<Rc<Self>, Error> {
        let sel = VPE::cur().alloc_sel();
        let (id, ndesc) = VPE::cur().resmng().unwrap().alloc_pe(sel, desc)?;
        Ok(Rc::new(PE {
            cap: Capability::new(sel, CapFlags::KEEP_CAP),
            id,
            desc: ndesc,
            free: true,
        }))
    }

    /// Binds a new PE object to given selector
    pub fn new_bind(id: PEId, desc: PEDesc, sel: Selector) -> Self {
        PE {
            cap: Capability::new(sel, CapFlags::KEEP_CAP),
            id,
            desc,
            free: false,
        }
    }

    /// Gets a PE with given description.
    ///
    /// The description is an '|' separated list of properties that will be tried in order. Two
    /// special properties are supported:
    /// - "own" to denote the own PE (provided that it has support for multiple VPEs)
    /// - "clone" to denote a separate PE that is identical to the own PE
    ///
    /// For other properties, see `PEDesc::derive`.
    ///
    /// Examples:
    /// - PE with an arbitrary ISA, but preferred the own: "own|core"
    /// - Identical PE, but preferred a separate one: "clone|own"
    /// - BOOM core if available, otherwise any core: "boom|core"
    /// - BOOM with NIC if available, otherwise a Rocket: "boom+nic|rocket"
    pub fn get(desc: &str) -> Result<Rc<Self>, Error> {
        let own = VPE::cur().pe();
        for props in desc.split('|') {
            match props {
                "own" => {
                    if own.desc().supports_pemux() && own.desc().has_virtmem() {
                        return Ok(own.clone());
                    }
                },
                "clone" => {
                    if let Ok(pe) = Self::new(own.desc()) {
                        return Ok(pe);
                    }
                },
                p => {
                    let base = PEDesc::new(own.desc().pe_type(), own.desc().isa(), 0);
                    if let Ok(pe) = Self::new(base.with_properties(p)) {
                        return Ok(pe);
                    }
                },
            }
        }
        Err(Error::new(Code::NotFound))
    }

    /// Derives a new PE object from `self` with a subset of the resources, removing them from `self`
    ///
    /// The three resources are the number of EPs, the time slice length in nanoseconds, and the
    /// number of page tables.
    pub fn derive(
        &self,
        eps: Option<u32>,
        time: Option<u64>,
        pts: Option<u64>,
    ) -> Result<Rc<Self>, Error> {
        let sel = VPE::cur().alloc_sel();
        syscalls::derive_pe(self.sel(), sel, eps, time, pts)?;
        Ok(Rc::new(PE {
            cap: Capability::new(sel, CapFlags::empty()),
            desc: self.desc(),
            id: self.id(),
            free: false,
        }))
    }

    /// Returns the selector
    pub fn sel(&self) -> Selector {
        self.cap.sel()
    }

    /// Returns the PE id
    pub fn id(&self) -> PEId {
        self.id
    }

    /// Returns the PE description
    pub fn desc(&self) -> PEDesc {
        self.desc
    }

    /// Returns the EP, time, and page table quota
    pub fn quota(&self) -> Result<(Quota<u32>, Quota<u64>, Quota<usize>), Error> {
        syscalls::pe_quota(self.sel())
    }

    /// Sets the quota of the PE with given selector to specified initial values (given time slice
    /// length and number of page tables).
    ///
    /// This call requires a root PE capability.
    pub fn set_quota(&self, time: u64, pts: u64) -> Result<(), Error> {
        syscalls::pe_set_quota(self.sel(), time, pts)
    }
}

impl Drop for PE {
    fn drop(&mut self) {
        if self.free {
            VPE::cur().resmng().unwrap().free_pe(self.sel()).ok();
        }
    }
}

impl fmt::Debug for PE {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(
            f,
            "PE{}[sel: {}, desc: {:?}]",
            self.id(),
            self.sel(),
            self.desc()
        )
    }
}
