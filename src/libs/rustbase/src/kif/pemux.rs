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

/// The VPE id of PEMux
pub const VPE_ID: u64 = 0xFFFF;
/// The VPE id when PEMux is idling
pub const IDLE_ID: u64 = 0xFFFE;

int_enum! {
    /// The upcalls from the kernel to PEMux
    pub struct Upcalls : u64 {
        const VPE_CTRL       = 0x0;
    }
}

pub use super::syscalls::VPEOp;

/// The VPE control upcall
#[repr(C, packed)]
pub struct VPECtrl {
    pub op: u64,
    pub pe_id: u64,
    pub vpe_sel: u64,
    pub vpe_op: u64,
}
