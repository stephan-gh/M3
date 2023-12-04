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
use crate::quota::Quota;
use crate::rc::Rc;
use crate::syscalls;

/// Represents a certain amount of kernel memory
///
/// Kernel memory is used by the M³ kernel to fulfill operations on behalf of activities. For
/// example, if the [`create_sgate`](`syscalls::create_sgate`) system call is used to create a new
/// [`SendGate`](`crate::com::SendGate`), the kernel needs memory to create the associated kernel
/// object. To prevent DOS attacks on the kernel, the kernel requires that this amount of memory is
/// available in the calling activity's kernel memory quota. This kernel memory quota is represented
/// by [`KMem`].
///
/// As the amount of kernel memory is fixed at boot, [`KMem`] instances cannot be created, but all
/// available memory is passed to the root activity, which distributes it among its children
/// accordingly. For that reason, [`KMem`] supports the [`derive`](`KMem::derive`) operation that
/// splits off a certain amount of kernel memory into a new [`KMem`] object.
pub struct KMem {
    cap: Capability,
}

impl KMem {
    /// Creates a new `KMem` object that is bound to given selector.
    pub fn new_bind(sel: Selector) -> Self {
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
        let sel = SelSpace::get().alloc_sel();

        syscalls::derive_kmem(self.sel(), sel, quota)?;
        Ok(Rc::new(KMem {
            cap: Capability::new(sel, CapFlags::empty()),
        }))
    }
}
