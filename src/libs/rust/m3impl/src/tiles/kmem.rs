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

use crate::cap::{CapFlags, Capability, Selector};
use crate::errors::Error;
use crate::quota::Quota;
use crate::rc::Rc;
use crate::syscalls;
use crate::tiles::Activity;

/// Represents a certain amount of kernel memory.
pub struct KMem {
    cap: Capability,
}

impl KMem {
    pub(crate) fn new(sel: Selector) -> Self {
        KMem {
            cap: Capability::new(sel, CapFlags::KEEP_CAP),
        }
    }

    /// Returns the capability selector.
    pub fn sel(&self) -> Selector {
        self.cap.sel()
    }

    /// Returns the total and remaining quota of the kernel memory.
    pub fn quota(&self) -> Result<Quota<usize>, Error> {
        syscalls::kmem_quota(self.sel())
    }

    /// Creates a new kernel memory object and transfers `quota` to the new object.
    pub fn derive(&self, quota: usize) -> Result<Rc<Self>, Error> {
        let sel = Activity::own().alloc_sel();

        syscalls::derive_kmem(self.sel(), sel, quota)?;
        Ok(Rc::new(KMem {
            cap: Capability::new(sel, CapFlags::empty()),
        }))
    }
}
