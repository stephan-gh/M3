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

use cap::{CapFlags, Capability, Selector};
use errors::Error;
use kif;
use syscalls;
use vpe::VPE;

#[derive(Debug)]
pub struct Semaphore {
    cap: Capability,
}

impl Semaphore {
    pub fn attach(name: &str) -> Result<Self, Error> {
        let sel = VPE::cur().alloc_sel();
        VPE::cur().resmng().use_sem(sel, name)?;

        Ok(Semaphore {
            cap: Capability::new(sel, CapFlags::KEEP_CAP),
        })
    }

    pub fn create(value: u32) -> Result<Self, Error> {
        let sel = VPE::cur().alloc_sel();
        syscalls::create_sem(sel, value)?;

        Ok(Semaphore {
            cap: Capability::new(sel, CapFlags::empty()),
        })
    }

    pub fn bind(sel: Selector) -> Self {
        Semaphore {
            cap: Capability::new(sel, CapFlags::KEEP_CAP),
        }
    }

    pub fn sel(&self) -> Selector {
        self.cap.sel()
    }

    pub fn up(&self) -> Result<(), Error> {
        syscalls::sem_ctrl(self.sel(), kif::syscalls::SemOp::UP)
    }

    pub fn down(&self) -> Result<(), Error> {
        syscalls::sem_ctrl(self.sel(), kif::syscalls::SemOp::DOWN)
    }
}
