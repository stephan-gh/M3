/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

use crate::cap::{CapFlags, Capability, SelSpace, Selector};
use crate::errors::Error;
use crate::kif;
use crate::syscalls;
use crate::tiles::Activity;

/// A syscall-based semaphore.
#[derive(Debug)]
pub struct Semaphore {
    cap: Capability,
}

impl Semaphore {
    /// Creates a new object that is attached to the global semaphore `name`.
    pub fn attach(name: &str) -> Result<Self, Error> {
        let sel = SelSpace::get().alloc_sel();
        Activity::own().resmng().unwrap().use_sem(sel, name)?;

        Ok(Semaphore {
            cap: Capability::new(sel, CapFlags::KEEP_CAP),
        })
    }

    /// Creates a new semaphore with the initial value `value`.
    pub fn create(value: u32) -> Result<Self, Error> {
        let sel = SelSpace::get().alloc_sel();
        syscalls::create_sem(sel, value)?;

        Ok(Semaphore {
            cap: Capability::new(sel, CapFlags::empty()),
        })
    }

    /// Binds a semaphore to the given selector.
    pub fn bind(sel: Selector) -> Self {
        Semaphore {
            cap: Capability::new(sel, CapFlags::KEEP_CAP),
        }
    }

    /// Returns the capability selector
    pub fn sel(&self) -> Selector {
        self.cap.sel()
    }

    /// Performs the `up` operation on the semaphore
    pub fn up(&self) -> Result<(), Error> {
        syscalls::sem_ctrl(self.sel(), kif::syscalls::SemOp::Up)
    }

    /// Performs the `down` operation on the semaphore
    pub fn down(&self) -> Result<(), Error> {
        syscalls::sem_ctrl(self.sel(), kif::syscalls::SemOp::Down)
    }
}
