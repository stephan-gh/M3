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
use base::col::Vec;
use base::kif;
use base::tcu::PEId;

use arch::ktcu;
use pes::PEMux;
use platform;

pub struct PEMng {
    muxes: Vec<PEMux>,
}

static INST: StaticCell<Option<PEMng>> = StaticCell::new(None);

pub fn init() {
    INST.set(Some(PEMng::new()));
}

pub fn get() -> &'static mut PEMng {
    INST.get_mut().as_mut().unwrap()
}

impl PEMng {
    fn new() -> Self {
        Self::deprivilege_pes();

        let mut muxes = Vec::new();
        for pe in platform::user_pes() {
            muxes.push(PEMux::new(pe));
        }
        PEMng { muxes }
    }

    pub fn pemux(&mut self, pe: PEId) -> &mut PEMux {
        assert!(pe > 0);
        &mut self.muxes[pe - 1]
    }

    pub fn find_pe(&mut self, pedesc: &kif::PEDesc) -> Option<PEId> {
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
}
