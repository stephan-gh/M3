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

use vpe;

static EPS: StaticCell<EPs> = StaticCell::new(EPs::new());

pub fn get() -> &'static mut EPs {
    EPS.get_mut()
}

pub struct EPs {
    gates: [Option<(u64, CapSel)>; dtu::EP_COUNT],
    reserved: u64,
    next_victim: usize,
}

impl EPs {
    const fn new() -> Self {
        EPs {
            gates: [None; dtu::EP_COUNT],
            reserved: 0,
            next_victim: dtu::FIRST_FREE_EP,
        }
    }

    pub fn is_free(&self, ep: EpId) -> bool {
        !self.is_reserved(ep) && self.gates[ep].is_none()
    }

    pub fn is_reserved(&self, ep: EpId) -> bool {
        (self.reserved & (1 << ep)) != 0
    }

    pub fn mark_reserved(&mut self, ep: EpId) {
        self.reserved |= 1 << ep;
    }

    pub fn mark_used(&mut self, vpe: u64, ep: EpId, gate: CapSel) {
        self.gates[ep] = Some((vpe, gate));
    }

    pub fn mark_free(&mut self, ep: EpId) {
        self.gates[ep] = None;
        self.reserved &= !(1 << ep);
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
        }
    }

    fn get_victim(&self) -> Result<EpId, Error> {
        for v in self.next_victim..dtu::EP_COUNT {
            if !self.is_reserved(v) {
                return Ok(v);
            }
        }
        for v in dtu::FIRST_FREE_EP..self.next_victim {
            if !self.is_reserved(v) {
                return Ok(v);
            }
        }
        Err(Error::new(Code::NoSpace))
    }
}
