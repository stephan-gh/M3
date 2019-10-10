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

use m3::cap::Selector;
use m3::cell::StaticCell;
use m3::col::{String, Vec};
use m3::errors::{Code, Error};
use m3::pes::VPE;
use m3::syscalls;

pub struct SemManager {
    sems: Vec<(String, Selector)>,
}

static MNG: StaticCell<SemManager> = StaticCell::new(SemManager::new());

pub fn get() -> &'static mut SemManager {
    MNG.get_mut()
}

impl SemManager {
    pub const fn new() -> Self {
        SemManager { sems: Vec::new() }
    }

    pub fn add_sem(&mut self, name: String) -> Result<(), Error> {
        if self.get(&name).is_some() {
            return Err(Error::new(Code::Exists));
        }

        let sel = VPE::cur().alloc_sel();
        syscalls::create_sem(sel, 0)?;

        log!(RESMNG_SEM, "Created semaphore {} @ {}", name, sel);
        self.sems.push((name, sel));
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<Selector> {
        for (sname, sel) in &self.sems {
            if sname == name {
                return Some(*sel);
            }
        }
        None
    }
}
