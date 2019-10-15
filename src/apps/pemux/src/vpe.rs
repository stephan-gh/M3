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
use base::col::Treap;
use base::dtu::{self, EpId};
use base::errors::{Code, Error};
use base::kif::{self, pemux, CapSel};
use base::util;

use eps;
use upcalls;
use IRQsOnGuard;

pub struct VPE {
    id: u64,
    gates: Treap<CapSel, EpId>,
}

static CUR: StaticCell<Option<VPE>> = StaticCell::new(None);

pub fn add(id: u64) {
    assert!((*CUR).is_none());

    log!(PEX_VPES, "Created VPE {}", id);
    CUR.set(Some(VPE::new(id)));
}

pub fn remove() {
    if (*CUR).is_some() {
        log!(PEX_VPES, "Destroyed VPE {}", (*CUR).as_ref().unwrap().id);
        CUR.set(None);
    }
}

pub fn get_vpe(id: u64) -> Option<&'static mut VPE> {
    match CUR.get_mut().as_mut() {
        Some(v) if v.id == id => Some(v),
        _ => None,
    }
}

pub fn cur() -> &'static mut VPE {
    CUR.get_mut().as_mut().unwrap()
}

impl VPE {
    pub fn new(id: u64) -> Self {
        let mut gates = Treap::new();
        gates.insert(kif::SEL_SYSC_SG, dtu::SYSC_SEP);
        gates.insert(kif::SEL_SYSC_RG, dtu::SYSC_REP);
        gates.insert(kif::SEL_UPC_RG, dtu::UPCALL_REP);
        gates.insert(kif::SEL_DEF_RG, dtu::DEF_REP);
        VPE { id, gates }
    }

    pub fn acquire_ep(&mut self, sel: CapSel) -> Result<EpId, Error> {
        if let Some(ep) = self.gates.get(&sel) {
            return Ok(*ep);
        }

        let ep = eps::get().find_free(false)?;
        // do that first to have a consistent state before activate
        self.add_gate(sel, ep);
        match self.activate(sel, ep, 0) {
            Ok(_) => Ok(ep),
            Err(e) => {
                self.remove_gate(sel, false);
                Err(e)
            },
        }
    }

    fn add_gate(&mut self, gate: CapSel, ep: EpId) {
        log!(PEX_VPES, "VPE{}: added {}->{:?}", self.id, gate, ep);
        eps::get().mark_used(self.id, ep, gate);
        self.gates.insert(gate, ep);
    }

    pub fn switch_gate(&mut self, ep: EpId, gate: CapSel) -> Result<(), Error> {
        if ep >= dtu::EP_COUNT || !eps::get().is_reserved_by(ep, self.id) {
            return Err(Error::new(Code::InvArgs));
        }

        if let Some(old_gate) = eps::get().gate_on(ep) {
            self.remove_gate(old_gate, false);
        }
        self.add_gate(gate, ep);
        Ok(())
    }

    pub fn remove_gate(&mut self, sel: CapSel, inval: bool) {
        if let Some(ep) = self.gates.remove(&sel) {
            log!(PEX_VPES, "VPE{}: removed {}->{}", self.id, sel, ep);
            eps::get().mark_free(ep);
            if inval {
                self.activate(kif::INVALID_SEL, ep, 0).ok();
            }
        }
    }

    fn activate(&mut self, sel: CapSel, ep: EpId, addr: usize) -> Result<(), Error> {
        let msg = pemux::Activate {
            op: pemux::KernReq::ACTIVATE.val as u64,
            vpe_sel: self.id,
            gate_sel: sel as u64,
            ep: ep as u64,
            addr: addr as u64,
        };

        let _irqs = IRQsOnGuard::new();
        dtu::DTU::send(
            dtu::KPEX_SEP,
            &msg as *const pemux::Activate as *const u8,
            util::size_of::<pemux::Activate>(),
            0,
            dtu::KPEX_REP,
        )?;

        // TODO do that asynchronously
        loop {
            let msg = dtu::DTU::fetch_msg(dtu::KPEX_REP);
            if let Some(m) = msg {
                let reply = unsafe { &*(&m.data as *const [u8] as *const [kif::DefaultReply]) };
                let res = reply[0].error;
                dtu::DTU::mark_read(dtu::KPEX_REP, m);
                return match res {
                    0 => Ok(()),
                    e => Err(Error::from(e as u32)),
                };
            }

            upcalls::check();

            dtu::DTU::sleep().ok();
        }
    }
}

impl Drop for VPE {
    fn drop(&mut self) {
        eps::get().remove_vpe(self.id);
    }
}
