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

use cap::{CapFlags, Selector};
use com::gate::Gate;
use com::EP;
use dtu::{self, EpId, EP_COUNT, FIRST_FREE_EP};
use errors::{Code, Error};
use kif::INVALID_SEL;
use syscalls;

/// The endpoint manager (`EpMng`) multiplexes all non-reserved endpoints among the gates.
pub struct EpMng {
    // remembers the mapping from gate to endpoint
    gates: [Option<Selector>; dtu::EP_COUNT],
    // whether we multiplex EPs
    multiplex: bool,
    // the next index in the gate array we use as a victim on multiplexing
    next_victim: usize,
    /// the reserved EPs
    reserved: u64,
}

impl EpMng {
    pub fn new(multiplex: bool) -> Self {
        EpMng {
            gates: [None; dtu::EP_COUNT],
            multiplex,
            next_victim: 1,
            reserved: 0,
        }
    }

    pub(crate) fn reserved(&self) -> u64 {
        self.reserved
    }

    /// Allocates a new endpoint and reserves it, that is, excludes it from multiplexing. Note that
    /// this can fail if a send gate with missing credits is using this EP.
    pub fn alloc_ep(&mut self) -> Result<EpId, Error> {
        for ep in FIRST_FREE_EP..EP_COUNT {
            if self.is_free(ep) {
                self.reserved |= 1 << ep;

                // take care that some non-fixed gate could already use that endpoint
                if self.multiplex && self.gates[ep].is_some() {
                    self.activate(ep, INVALID_SEL).ok();
                }
                self.gates[ep] = None;

                return Ok(ep);
            }
        }
        Err(Error::new(Code::NoSpace))
    }

    /// Frees the given endpoint
    pub fn free_ep(&mut self, id: EpId) {
        self.reserved &= !(1 << id);
    }

    pub(crate) fn set_owned(&mut self, ep: EpId, sel: Selector) {
        self.gates[ep] = Some(sel);
    }

    pub(crate) fn set_unowned(&mut self, ep: EpId) {
        self.gates[ep] = None;
    }

    pub(crate) fn reset(&mut self, eps: u64) {
        for ep in 0..dtu::EP_COUNT {
            self.gates[ep] = None;
        }
        self.reserved = eps;
    }

    /// Returns true if the endpoint `ep` is owned by the gate with selector `sel`.
    pub fn ep_owned_by(&self, ep: EpId, sel: Selector) -> bool {
        match self.gates[ep] {
            Some(s) => s == sel,
            None => false,
        }
    }

    /// Activates the given gate. If there is no free endpoint available, another gate will be
    /// deactivated. Returns the chosen endpoint number.
    pub fn switch_to(&mut self, g: &Gate) -> Result<EpId, Error> {
        let idx = self.select_victim()?;
        self.activate(idx, g.sel())?;
        g.set_epid(idx);
        self.gates[idx] = Some(g.sel());
        Ok(idx)
    }

    /// Removes the given gate from `EpMng`.
    pub fn remove(&mut self, g: &Gate) {
        if let Some(ep) = g.ep() {
            if self.ep_owned_by(ep, g.sel()) {
                self.gates[ep] = None;
                // only necessary if we won't revoke the gate anyway
                if !(g.flags() & CapFlags::KEEP_CAP).is_empty() {
                    self.activate(ep, INVALID_SEL).ok();
                }
            }
        }
    }

    fn is_free(&self, id: EpId) -> bool {
        id >= dtu::FIRST_FREE_EP && (self.reserved & (1 << id)) == 0
    }

    fn select_victim(&mut self) -> Result<EpId, Error> {
        let mut victim = self.next_victim;
        for _ in 0..dtu::EP_COUNT {
            if self.is_free(victim) {
                break;
            }

            victim = (victim + 1) % dtu::EP_COUNT;
        }

        if !self.is_free(victim) {
            Err(Error::new(Code::NoSpace))
        }
        else {
            self.next_victim = (victim + 1) % dtu::EP_COUNT;
            Ok(victim)
        }
    }

    fn activate(&self, ep: EpId, gate: Selector) -> Result<(), Error> {
        syscalls::activate(EP::sel_of(ep), gate, 0)
    }
}
