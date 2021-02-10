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

use m3::cell::{RefCell, StaticCell};
use m3::col::Vec;
use m3::com::MemGate;
use m3::errors::{Code, Error};
use m3::kif::{PEDesc, Perm};
use m3::log;
use m3::pes::{PE, VPE};
use m3::rc::Rc;
use m3::syscalls;
use m3::tcu::{EpId, PEId, PMEM_PROT_EPS, TCU};

struct ManagedPE {
    id: PEId,
    pe: Rc<PE>,
    users: u32,
}

struct PMP {
    next_ep: EpId,
    regions: Vec<(MemGate, usize)>,
}

impl PMP {
    fn new() -> Self {
        Self {
            // PMP EPs start at 1, because 0 is reserved for PEMux
            next_ep: 1,
            regions: Vec::new(),
        }
    }
}

pub struct PEUsage {
    idx: Option<usize>,
    pmp: Rc<RefCell<PMP>>,
    pe: Rc<PE>,
}

impl PEUsage {
    fn new(idx: usize) -> Self {
        Self {
            idx: Some(idx),
            pmp: Rc::new(RefCell::new(PMP::new())),
            pe: get().get(idx),
        }
    }

    pub fn new_obj(pe: Rc<PE>) -> Self {
        Self {
            idx: None,
            pmp: Rc::new(RefCell::new(PMP::new())),
            pe,
        }
    }

    pub fn pe_id(&self) -> PEId {
        self.pe.id()
    }

    pub fn pe_obj(&self) -> &Rc<PE> {
        &self.pe
    }

    pub fn add_mem_region(&self, mgate: MemGate, size: usize, set: bool) -> Result<(), Error> {
        let mut pmp = self.pmp.borrow_mut();
        if set {
            syscalls::set_pmp(self.pe_obj().sel(), mgate.sel(), pmp.next_ep)?;
            pmp.next_ep += 1;
        }
        pmp.regions.push((mgate, size));
        Ok(())
    }

    pub fn inherit_mem_regions(&self, pe: &Rc<PEUsage>) -> Result<(), Error> {
        let pmps = pe.pmp.borrow();
        for (mgate, size) in pmps.regions.iter() {
            self.add_mem_region(mgate.derive(0, *size, Perm::RWX)?, *size, true)?;
        }
        Ok(())
    }

    pub fn derive(&self, eps: u32) -> Result<PEUsage, Error> {
        let pe = self.pe_obj().derive(eps)?;
        if let Some(idx) = self.idx {
            get().pes[idx].users += 1;
        }
        log!(
            crate::LOG_PES,
            "Deriving PE{}: (eps={})",
            self.pe_id(),
            pe.quota().unwrap(),
        );
        Ok(PEUsage {
            idx: self.idx,
            pmp: self.pmp.clone(),
            pe,
        })
    }
}

impl Drop for PEUsage {
    fn drop(&mut self) {
        if let Some(idx) = self.idx {
            get().free(idx);
        }
    }
}

pub struct PEManager {
    pes: Vec<ManagedPE>,
}

static MNG: StaticCell<PEManager> = StaticCell::new(PEManager::new());

pub fn get() -> &'static mut PEManager {
    MNG.get_mut()
}

impl PEManager {
    pub const fn new() -> Self {
        PEManager { pes: Vec::new() }
    }

    pub fn add(&mut self, id: PEId, pe: Rc<PE>) {
        self.pes.push(ManagedPE { id, pe, users: 0 });
    }

    pub fn count(&self) -> usize {
        self.pes.len()
    }

    pub fn id(&self, idx: usize) -> PEId {
        self.pes[idx].id
    }

    pub fn get(&self, idx: usize) -> Rc<PE> {
        self.pes[idx].pe.clone()
    }

    pub fn find<F>(&self, f: F) -> Option<usize>
    where
        F: Fn(&Rc<PE>) -> bool,
    {
        self.pes.iter().position(|p| p.users == 0 && f(&p.pe))
    }

    pub fn find_and_alloc(&mut self, desc: PEDesc) -> Result<PEUsage, Error> {
        for (id, pe) in self.pes.iter().enumerate() {
            if pe.users == 0
                && pe.pe.desc().isa() == desc.isa()
                && pe.pe.desc().pe_type() == desc.pe_type()
            {
                let usage = PEUsage::new(id);
                if pe.id == VPE::cur().pe_id() {
                    // if it's our own PE, set it to the first free PMP EP
                    let mut pmp = usage.pmp.borrow_mut();
                    for ep in pmp.next_ep..PMEM_PROT_EPS as EpId {
                        if !TCU::is_valid(ep) {
                            break;
                        }
                        pmp.next_ep += 1;
                    }
                }
                self.alloc(id);
                return Ok(usage);
            }
        }
        Err(Error::new(Code::NoSpace))
    }

    pub fn alloc(&mut self, idx: usize) {
        log!(
            crate::LOG_PES,
            "Allocating PE{}: {:?} (eps={})",
            self.pes[idx].id,
            self.pes[idx].pe.desc(),
            self.get(idx).quota().unwrap(),
        );
        self.pes[idx].users += 1;
    }

    fn free(&mut self, idx: usize) {
        let mut pe = &mut self.pes[idx];
        pe.users -= 1;
        if pe.users == 0 {
            log!(crate::LOG_PES, "Freeing PE{}: {:?}", pe.id, pe.pe.desc());
        }
    }
}
