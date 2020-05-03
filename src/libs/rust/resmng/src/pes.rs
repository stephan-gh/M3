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

use m3::cell::StaticCell;
use m3::col::Vec;
use m3::errors::{Code, Error};
use m3::kif::PEDesc;
use m3::pes::PE;
use m3::rc::Rc;
use m3::tcu::PEId;

struct ManagedPE {
    id: PEId,
    pe: Rc<PE>,
    users: u32,
}

pub struct PEUsage {
    idx: usize,
    pe: Option<Rc<PE>>,
}

impl PEUsage {
    fn new(idx: usize) -> Self {
        Self { idx, pe: None }
    }

    pub fn pe_id(&self) -> PEId {
        get().pes[self.idx].id
    }

    pub fn pe_obj(&self) -> Rc<PE> {
        match self.pe {
            Some(ref p) => p.clone(),
            None => get().get(self.idx),
        }
    }

    pub fn derive(&self, eps: u32) -> Result<PEUsage, Error> {
        let pe = self.pe_obj().derive(eps)?;
        get().pes[self.idx].users += 1;
        log!(
            crate::LOG_PES,
            "Deriving PE{}: (eps={})",
            get().pes[self.idx].id,
            pe.quota().unwrap(),
        );
        Ok(PEUsage {
            idx: self.idx,
            pe: Some(pe),
        })
    }
}

impl Drop for PEUsage {
    fn drop(&mut self) {
        get().free(self.idx);
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

    pub fn get(&self, id: usize) -> Rc<PE> {
        self.pes[id].pe.clone()
    }

    pub fn find_and_alloc(&mut self, desc: PEDesc) -> Result<PEUsage, Error> {
        for (id, pe) in self.pes.iter().enumerate() {
            if pe.users == 0
                && pe.pe.desc().isa() == desc.isa()
                && pe.pe.desc().pe_type() == desc.pe_type()
            {
                self.alloc(id);
                return Ok(PEUsage::new(id));
            }
        }
        Err(Error::new(Code::NoSpace))
    }

    fn alloc(&mut self, id: usize) {
        log!(
            crate::LOG_PES,
            "Allocating PE{}: {:?} (eps={})",
            self.pes[id].id,
            self.pes[id].pe.desc(),
            self.get(id).quota().unwrap(),
        );
        self.pes[id].users += 1;
    }

    fn free(&mut self, id: usize) {
        let mut pe = &mut self.pes[id];
        pe.users -= 1;
        if pe.users == 0 {
            log!(crate::LOG_PES, "Freeing PE{}: {:?}", pe.id, pe.pe.desc());
        }
    }
}
