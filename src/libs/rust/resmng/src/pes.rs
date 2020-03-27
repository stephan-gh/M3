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
    used: bool,
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
        self.pes.push(ManagedPE {
            id,
            pe,
            used: false,
        });
    }

    pub fn len(&self) -> usize {
        self.pes.len()
    }

    pub fn pe(&self, id: usize) -> PEId {
        self.pes[id].id
    }

    pub fn get(&self, id: usize) -> Rc<PE> {
        self.pes[id].pe.clone()
    }

    pub fn find(&self, desc: PEDesc) -> Result<usize, Error> {
        for (id, pe) in self.pes.iter().enumerate() {
            if !pe.used
                && pe.pe.desc().isa() == desc.isa()
                && pe.pe.desc().pe_type() == desc.pe_type()
            {
                return Ok(id);
            }
        }
        Err(Error::new(Code::NoSpace))
    }

    pub fn find_and_alloc(&mut self, desc: PEDesc) -> Result<usize, Error> {
        self.find(desc).map(|id| {
            self.alloc(id);
            id
        })
    }

    pub fn alloc(&mut self, id: usize) {
        log!(
            crate::LOG_PES,
            "Allocating PE{}: {:?}",
            self.pes[id].id,
            self.pes[id].pe.desc()
        );
        self.pes[id].used = true;
    }

    pub fn free(&mut self, id: usize) {
        let mut pe = &mut self.pes[id];
        log!(crate::LOG_PES, "Freeing PE{}: {:?}", pe.id, pe.pe.desc());
        pe.used = false;
    }
}
