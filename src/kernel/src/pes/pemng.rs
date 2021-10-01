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

use base::cell::{LazyStaticRefCell, RefMut};
use base::col::Vec;
use base::kif;
use base::tcu::PEId;

use crate::arch::ktcu;
use crate::pes::PEMux;
use crate::platform;

static INST: LazyStaticRefCell<Vec<PEMux>> = LazyStaticRefCell::default();

pub fn init() {
    deprivilege_pes();

    let mut muxes = Vec::new();
    for pe in platform::user_pes() {
        muxes.push(PEMux::new(pe));
    }
    INST.set(muxes);
}

pub fn pemux(pe: PEId) -> RefMut<'static, PEMux> {
    assert!(pe > 0);
    RefMut::map(INST.borrow_mut(), |pes| &mut pes[pe as usize - 1])
}

pub fn find_pe(pedesc: &kif::PEDesc) -> Option<PEId> {
    for pe in platform::user_pes() {
        if platform::pe_desc(pe).isa() == pedesc.isa()
            || platform::pe_desc(pe).pe_type() == pedesc.pe_type()
        {
            return Some(pe);
        }
    }

    None
}

fn deprivilege_pes() {
    for pe in platform::user_pes() {
        ktcu::deprivilege_pe(pe).expect("Unable to deprivilege PE");
    }
}
