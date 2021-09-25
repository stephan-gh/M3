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

use m3::cell::{RefCell, StaticUnsafeCell};
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
            pe.quota().unwrap().1,
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

// TODO can we use a safe cell here?
static MNG: StaticUnsafeCell<PEManager> = StaticUnsafeCell::new(PEManager::new());

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

    pub fn find_with_desc(&mut self, desc: &str) -> Option<usize> {
        let own = VPE::cur().pe().desc();
        for props in desc.split('|') {
            let base = PEDesc::new(own.pe_type(), own.isa(), 0);
            if let Ok(idx) = self.find(base.with_properties(props)) {
                return Some(idx);
            }
        }
        log!(crate::LOG_PES, "Unable to find PE with desc {}", desc);
        None
    }

    pub fn find_and_alloc_with_desc(&mut self, desc: &str) -> Result<PEUsage, Error> {
        let own = VPE::cur().pe().desc();
        for props in desc.split('|') {
            let base = PEDesc::new(own.pe_type(), own.isa(), 0);
            if let Ok(pe) = self.find_and_alloc(base.with_properties(props)) {
                return Ok(pe);
            }
        }
        log!(crate::LOG_PES, "Unable to find PE with desc {}", desc);
        Err(Error::new(Code::NotFound))
    }

    pub fn find_and_alloc(&mut self, desc: PEDesc) -> Result<PEUsage, Error> {
        self.find(desc).map(|idx| {
            let usage = PEUsage::new(idx);
            if self.pes[idx].id == VPE::cur().pe_id() {
                // if it's our own PE, set it to the first free PMP EP
                let mut pmp = usage.pmp.borrow_mut();
                for ep in pmp.next_ep..PMEM_PROT_EPS as EpId {
                    if !TCU::is_valid(ep) {
                        break;
                    }
                    pmp.next_ep += 1;
                }
            }
            self.alloc(idx);
            usage
        })
    }

    fn find(&mut self, desc: PEDesc) -> Result<usize, Error> {
        for (id, pe) in self.pes.iter().enumerate() {
            if pe.users == 0
                && pe.pe.desc().isa() == desc.isa()
                && pe.pe.desc().pe_type() == desc.pe_type()
                && (desc.attr().is_empty() || pe.pe.desc().attr() == desc.attr())
            {
                return Ok(id);
            }
        }
        Err(Error::new(Code::NotFound))
    }

    pub fn alloc(&mut self, idx: usize) {
        log!(
            crate::LOG_PES,
            "Allocating PE{}: {:?} (eps={})",
            self.pes[idx].id,
            self.pes[idx].pe.desc(),
            self.get(idx).quota().unwrap().1,
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
