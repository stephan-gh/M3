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
use cell::StaticCell;
use com::gate::Gate;
use dtu::{self, EpId};
use errors::{Code, Error};
use kif::INVALID_SEL;
use syscalls;
use vpe;

pub struct EpMux {
    gates: [Option<Selector>; dtu::EP_COUNT],
    next_victim: usize,
}

static EP_MUX: StaticCell<EpMux> = StaticCell::new(EpMux::new());

impl EpMux {
    const fn new() -> Self {
        EpMux {
            gates: [None; dtu::EP_COUNT],
            next_victim: 1,
        }
    }

    pub fn get() -> &'static mut EpMux {
        EP_MUX.get_mut()
    }

    pub fn reserve(&mut self, ep: EpId) {
        // take care that some non-fixed gate could already use that endpoint
        if let Some(_) = self.gates[ep] {
            self.activate(ep, INVALID_SEL).ok();
        }
        self.gates[ep] = None;
    }

    pub(crate) fn set_owned(&mut self, ep: EpId, sel: Selector) {
        self.gates[ep] = Some(sel);
    }
    pub(crate) fn unset_owned(&mut self, ep: EpId) {
        self.gates[ep] = None;
    }

    pub fn ep_owned_by(&self, ep: EpId, sel: Selector) -> bool {
        match self.gates[ep] {
            Some(s) => s == sel,
            None    => false
        }
    }

    pub fn switch_to(&mut self, g: &Gate) -> Result<EpId, Error> {
        let idx = self.select_victim()?;
        self.activate(idx, g.sel())?;
        g.set_ep(idx);
        Ok(idx)
    }

    pub fn switch_cap(&mut self, g: &Gate, sel: Selector) -> Result<(), Error> {
        if let Some(ep) = g.ep() {
            if self.ep_owned_by(ep, g.sel()) {
                self.activate(ep, sel)?;
                if sel == INVALID_SEL {
                    self.gates[ep] = None;
                }
            }
        }
        Ok(())
    }

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

    pub fn reset(&mut self) {
        for ep in 0..dtu::EP_COUNT {
            self.gates[ep] = None;
        }
    }

    fn select_victim(&mut self) -> Result<EpId, Error> {
        let mut victim = self.next_victim;
        for _ in 0..dtu::EP_COUNT {
            if vpe::VPE::cur().is_ep_free(victim) {
                break;
            }

            victim = (victim + 1) % dtu::EP_COUNT;
        }

        if !vpe::VPE::cur().is_ep_free(victim) {
            Err(Error::new(Code::NoSpace))
        }
        else {
            self.next_victim = (victim + 1) % dtu::EP_COUNT;
            Ok(victim)
        }
    }

    fn activate(&self, ep: EpId, gate: Selector) -> Result<(), Error> {
        syscalls::activate(vpe::VPE::cur().ep_sel(ep), gate, 0)
    }
}
