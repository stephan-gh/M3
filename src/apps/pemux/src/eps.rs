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

use base::cell::StaticCell;
use base::dtu::{self, EpId};
use base::errors::{Code, Error};
use base::kif::CapSel;
use core::mem;

use vpe;

static EPS: StaticCell<EPs> = StaticCell::new(EPs::new());

pub fn get() -> &'static mut EPs {
    EPS.get_mut()
}

pub struct EPs {
    gates: [Option<(u64, CapSel)>; dtu::EP_COUNT],
    reserved: [Option<u64>; dtu::EP_COUNT],
    next_victim: usize,
}

impl EPs {
    const fn new() -> Self {
        EPs {
            gates: [None; dtu::EP_COUNT],
            reserved: [None; dtu::EP_COUNT],
            next_victim: dtu::FIRST_FREE_EP,
        }
    }

    pub fn gate_on(&self, ep: EpId) -> Option<CapSel> {
        self.gates[ep].map(|(_, sel)| sel)
    }

    pub fn is_free(&self, ep: EpId) -> bool {
        self.reserved[ep].is_none() && self.gates[ep].is_none()
    }

    pub fn is_reserved_by(&self, ep: EpId, vpe: u64) -> bool {
        match self.reserved[ep] {
            Some(id) if id == vpe => true,
            _ => false,
        }
    }

    pub fn mark_reserved(&mut self, ep: EpId, vpe: u64) {
        self.reserved[ep] = Some(vpe);
    }

    pub fn mark_unreserved(&mut self, ep: EpId) -> Option<(u64, CapSel)> {
        self.reserved[ep] = None;
        mem::replace(&mut self.gates[ep], None)
    }

    pub fn mark_used(&mut self, vpe: u64, ep: EpId, gate: CapSel) {
        self.gates[ep] = Some((vpe, gate));
    }

    pub fn mark_free(&mut self, ep: EpId) {
        self.gates[ep] = None;
    }

    pub fn find_free(&mut self, inval: bool) -> Result<EpId, Error> {
        for ep in dtu::FIRST_FREE_EP..dtu::EP_COUNT {
            if self.is_free(ep) {
                return Ok(ep);
            }
        }

        let victim = self.get_victim()?;
        let (_, gate) = self.gates[victim].take().unwrap();
        vpe::cur().remove_gate(gate, inval);
        if self.next_victim + 1 < dtu::EP_COUNT {
            self.next_victim += 1;
        }
        else {
            self.next_victim = dtu::FIRST_FREE_EP;
        }
        Ok(victim)
    }

    pub fn remove_vpe(&mut self, vpe: u64) {
        for ep in dtu::FIRST_FREE_EP..dtu::EP_COUNT {
            if let Some((v, _)) = self.gates[ep] {
                if vpe == v {
                    self.mark_free(ep);
                }
            }
            if let Some(v) = self.reserved[ep] {
                if vpe == v {
                    self.reserved[ep] = None;
                }
            }
        }
    }

    fn get_victim(&self) -> Result<EpId, Error> {
        for v in self.next_victim..dtu::EP_COUNT {
            if self.reserved[v].is_none() && !dtu::DTU::has_missing_credits(v) {
                return Ok(v);
            }
        }
        for v in dtu::FIRST_FREE_EP..self.next_victim {
            if self.reserved[v].is_none() && !dtu::DTU::has_missing_credits(v) {
                return Ok(v);
            }
        }
        Err(Error::new(Code::NoSpace))
    }
}
