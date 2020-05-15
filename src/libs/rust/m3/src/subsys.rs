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

use cap::Selector;
use col::Vec;
use com::MemGate;
use errors::Error;
use goff;
use kif::{boot, FIRST_FREE_SEL};
use pes::PE;
use rc::Rc;
use util;

//
// Our parent/kernel initializes our cap space as follows:
// +-----------+-------+-----+-----------+------+-----+----------+-------+-----+-----------+
// | boot info | mod_0 | ... | mod_{n-1} | pe_0 | ... | pe_{n-1} | mem_0 | ... | mem_{n-1} |
// +-----------+-------+-----+-----------+------+-----+----------+-------+-----+-----------+
// ^-- FIRST_FREE_SEL
//
const SUBSYS_SELS: Selector = FIRST_FREE_SEL;

pub struct Subsystem {
    info: boot::Info,
    mods: Vec<boot::Mod>,
    pes: Vec<boot::PE>,
    mems: Vec<boot::Mem>,
}

impl Subsystem {
    pub fn new() -> Result<Self, Error> {
        let mgate = MemGate::new_bind(SUBSYS_SELS);
        let mut off: goff = 0;

        let info: boot::Info = mgate.read_obj(0)?;
        off += util::size_of::<boot::Info>() as goff;

        let mut mods = Vec::<boot::Mod>::with_capacity(info.mod_count as usize);
        // safety: will be initialized by read below
        unsafe { mods.set_len(info.mod_count as usize) };
        mgate.read(&mut mods, off)?;
        off += util::size_of::<boot::Mod>() as goff * info.mod_count;

        let mut pes = Vec::<boot::PE>::with_capacity(info.pe_count as usize);
        // safety: will be initialized by read below
        unsafe { pes.set_len(info.pe_count as usize) };
        mgate.read(&mut pes, off)?;
        off += util::size_of::<boot::PE>() as goff * info.pe_count;

        let mut mems = Vec::<boot::Mem>::with_capacity(info.mem_count as usize);
        // safety: will be initialized by read below
        unsafe { mems.set_len(info.mem_count as usize) };
        mgate.read(&mut mems, off)?;

        Ok(Self { info, mods, pes, mems })
    }

    pub fn info(&self) -> &boot::Info {
        &self.info
    }

    pub fn mods(&self) -> &Vec<boot::Mod> {
        &self.mods
    }

    pub fn pes(&self) -> &Vec<boot::PE> {
        &self.pes
    }

    pub fn mems(&self) -> &Vec<boot::Mem> {
        &self.mems
    }

    pub fn get_mod(&self, idx: usize) -> MemGate {
        MemGate::new_bind(SUBSYS_SELS + 1 + idx as Selector)
    }

    pub fn get_pe(&self, idx: usize) -> Rc<PE> {
        Rc::new(PE::new_bind(
            self.pes[idx].desc,
            SUBSYS_SELS + 1 + (self.info.mod_count as usize + idx) as Selector,
        ))
    }

    pub fn get_mem(&self, idx: usize) -> MemGate {
        MemGate::new_bind(
            SUBSYS_SELS + 1 + (self.info.mod_count as usize + self.pes.len() + idx) as Selector,
        )
    }
}
