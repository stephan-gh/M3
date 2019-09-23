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
use base::kif::{self, kpexcalls, CapSel};
use base::util;

use eps;

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
            log!(
                PEX_EPS,
                "VPE{}: Using EP {} for Gate {:?}",
                self.id,
                *ep,
                sel
            );
            return Ok(*ep);
        }

        let ep = eps::get().find_free(false)?;
        self.activate(sel, ep, 0)?;
        self.add_gate(sel, ep);
        Ok(ep)
    }

    pub fn reserve_ep(&mut self, ep: Option<EpId>) -> Result<EpId, Error> {
        if let Some(id) = ep {
            if eps::get().is_free(id) {
                Ok(id)
            }
            else {
                Err(Error::new(Code::Exists))
            }
        }
        else {
            eps::get().find_free(true)
        }
        .map(|epid| {
            log!(PEX_EPS, "VPE{}: reserving EP {}", self.id, epid);
            // the application can refer to this EP with this special gate id
            self.add_gate(1 << 31 | epid as u32, epid);
            eps::get().mark_reserved(epid);
            epid
        })
    }

    pub fn free_ep(&mut self, ep: EpId) -> Result<(), Error> {
        if eps::get().is_free(ep) {
            return Err(Error::new(Code::InvArgs));
        }

        log!(PEX_EPS, "VPE{}: freeing EP {}", self.id, ep);
        eps::get().mark_free(ep);
        Ok(())
    }

    pub fn activate_gate(&mut self, gate: CapSel, ep: EpId, addr: usize) -> Result<(), Error> {
        // the EP needs to be reserved first
        if self.gates.get(&(1 << 31 | ep as u32)).is_none() {
            return Err(Error::new(Code::InvArgs));
        }

        log!(
            PEX_VPES,
            "VPE{}: activating gate {} on EP {}",
            self.id,
            gate,
            ep
        );
        self.activate(gate, ep, addr)?;
        self.add_gate(gate, ep);
        eps::get().mark_reserved(ep);
        Ok(())
    }

    fn add_gate(&mut self, gate: CapSel, ep: EpId) {
        log!(PEX_VPES, "VPE{}: added {}->{:?}", self.id, gate, ep);
        eps::get().mark_used(self.id, ep, gate);
        self.gates.insert(gate, ep);
    }

    pub fn remove_gate(&mut self, sel: CapSel, inval: bool) {
        if let Some(ep) = self.gates.remove(&sel) {
            log!(PEX_VPES, "VPE{}: removed {}->{}", self.id, sel, ep);
            if inval {
                self.activate(kif::INVALID_SEL, ep, 0).ok();
            }
        }
    }

    fn activate(&mut self, sel: CapSel, ep: EpId, addr: usize) -> Result<(), Error> {
        let msg = kpexcalls::Activate {
            op: kpexcalls::Operation::ACTIVATE.val as u64,
            vpe_sel: self.id,
            gate_sel: sel as u64,
            ep: ep as u64,
            addr: addr as u64,
        };
        dtu::DTU::send(
            dtu::KPEX_SEP,
            &msg as *const kpexcalls::Activate as *const u8,
            util::size_of::<kpexcalls::Activate>(),
            0,
            dtu::KPEX_REP,
        )?;

        // TODO do that asynchronously
        loop {
            let msg = dtu::DTU::fetch_msg(dtu::KPEX_REP);
            if let Some(m) = msg {
                let reply: &[kif::syscalls::DefaultReply] =
                    unsafe { &*(&m.data as *const [u8] as *const [kif::syscalls::DefaultReply]) };
                let res = reply[0].error;
                dtu::DTU::mark_read(dtu::KPEX_REP, m);
                return match res {
                    0 => Ok(()),
                    e => Err(Error::from(e as u32)),
                };
            }
        }
    }
}

impl Drop for VPE {
    fn drop(&mut self) {
        eps::get().remove_vpe(self.id);
    }
}
