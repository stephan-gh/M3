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

//! The kernel-pemux interface

int_enum! {
    /// The kernel requests
    pub struct KernReq : u64 {
        const ACTIVATE      = 0x0;
    }
}

/// The activate request message
#[repr(C, packed)]
pub struct Activate {
    pub op: u64,
    pub vpe_sel: u64,
    pub gate_sel: u64,
    pub ep: u64,
    pub addr: u64,
}

int_enum! {
    /// The upcalls from the kernel to PEMux
    pub struct Upcalls : u64 {
        const ALLOC_EP      = 0x0;
        const FREE_EP       = 0x1;
    }
}

/// The alloc endpoint upcall
#[repr(C, packed)]
pub struct AllocEP {
    pub op: u64,
    pub vpe_sel: u64,
}

/// The alloc endpoint reply
#[repr(C, packed)]
pub struct AllocEPReply {
    pub error: u64,
    pub ep: u64,
}

/// The free endpoint upcall
#[repr(C, packed)]
pub struct FreeEP {
    pub op: u64,
    pub ep: u64,
}
