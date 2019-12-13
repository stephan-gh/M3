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
use base::dtu;
use base::kif;

use vma;

pub struct VPE {
    id: u64,
    vpe_reg: dtu::Reg,
    root_pt: u64,
}

static CUR: StaticCell<Option<VPE>> = StaticCell::new(None);
static IDLE: StaticCell<VPE> = StaticCell::new(VPE::new(kif::pemux::IDLE_ID, 0));
static OWN: StaticCell<VPE> = StaticCell::new(VPE::new(kif::pemux::VPE_ID, 0));

pub fn init() {
    let root_pt = vma::get_addr_space();
    IDLE.get_mut().root_pt = root_pt;
    OWN.get_mut().root_pt = root_pt;
}

pub fn add(id: u64, root_pt: u64) {
    assert!((*CUR).is_none());

    log!(PEX_VPES, "Created VPE {}", id);

    // switch to the address space
    if root_pt != 0 {
        vma::set_addr_space(root_pt);
        dtu::DTU::invalidate_tlb();
    }

    CUR.set(Some(VPE::new(id, root_pt)));
}

pub fn get_mut(id: u64) -> Option<&'static mut VPE> {
    if id == kif::pemux::VPE_ID {
        return Some(our());
    }
    else {
        let c = cur();
        if c.id == id {
            return Some(c);
        }
    }
    None
}

pub fn our() -> &'static mut VPE {
    OWN.get_mut()
}

pub fn cur() -> &'static mut VPE {
    match CUR.get_mut() {
        Some(v) => v,
        None => IDLE.get_mut(),
    }
}

pub fn remove() {
    if (*CUR).is_some() {
        let old = CUR.set(None).unwrap();
        log!(PEX_VPES, "Destroyed VPE {}", old.id);

        // switch back to our own address space
        if old.root_pt != 0 {
            vma::set_addr_space(our().root_pt());
            dtu::DTU::invalidate_tlb();
        }
    }
}

impl VPE {
    pub const fn new(id: u64, root_pt: u64) -> Self {
        VPE {
            id,
            vpe_reg: id << 19,
            root_pt,
        }
    }

    pub fn root_pt(&self) -> u64 {
        self.root_pt
    }

    pub fn vpe_reg(&self) -> dtu::Reg {
        self.vpe_reg
    }

    pub fn set_vpe_reg(&mut self, val: dtu::Reg) {
        self.vpe_reg = val;
    }

    pub fn msgs(&self) -> u16 {
        ((self.vpe_reg >> 3) & 0xFFFF) as u16
    }

    pub fn has_msgs(&self) -> bool {
        self.msgs() != 0
    }

    pub fn add_msg(&mut self) {
        self.vpe_reg += 1 << 3;
    }
}
