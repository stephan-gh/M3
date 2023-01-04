/*
 * Copyright (C) 2019-2021 Nils Asmussen, Barkhausen Institut
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

use m3::col::{String, Vec};
use m3::com::Semaphore;
use m3::errors::{Code, Error};
use m3::log;

#[derive(Default)]
pub struct SemManager {
    sems: Vec<(String, Semaphore)>,
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
