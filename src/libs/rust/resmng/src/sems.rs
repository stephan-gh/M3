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
use m3::col::{String, Vec};
use m3::com::Semaphore;
use m3::errors::{Code, Error};
use m3::log;

pub struct SemManager {
    sems: Vec<(String, Semaphore)>,
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

        let sem = Semaphore::create(0)?;
        log!(crate::LOG_SEM, "Created semaphore {} @ {}", name, sem.sel());
        self.sems.push((name, sem));
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&Semaphore> {
        for (sname, sem) in &self.sems {
            if sname == name {
                return Some(sem);
            }
        }
        None
    }
}
