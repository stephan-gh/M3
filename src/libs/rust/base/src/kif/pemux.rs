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
        const MAP            = 0x1;
        const TRANSLATE      = 0x2;
        const REM_MSGS       = 0x3;
        const EP_INVAL       = 0x4;
    }
}

int_enum! {
    /// The operations for the `vpe_ctrl` upcall
    pub struct VPEOp : u64 {
        const INIT  = 0x0;
        const START = 0x1;
        const STOP  = 0x2;
    }
}

/// The VPE control upcall
#[repr(C, packed)]
pub struct VPECtrl {
    pub op: u64,
    pub vpe_sel: u64,
    pub vpe_op: u64,
    pub eps_start: u64,
}

/// The map upcall
#[repr(C, packed)]
pub struct Map {
    pub op: u64,
    pub vpe_sel: u64,
    pub virt: u64,
    pub global: u64,
    pub pages: u64,
    pub perm: u64,
}

/// The translate upcall
#[repr(C, packed)]
pub struct Translate {
    pub op: u64,
    pub vpe_sel: u64,
    pub virt: u64,
    pub perm: u64,
}

/// The remove messages upcall
#[repr(C, packed)]
pub struct RemMsgs {
    pub op: u64,
    pub vpe_sel: u64,
    pub unread_mask: u64,
}

/// The EP invalidation upcall
#[repr(C, packed)]
pub struct EpInval {
    pub op: u64,
    pub vpe_sel: u64,
    pub ep: u64,
}

/// The upcall response
#[repr(C, packed)]
pub struct Response {
    pub error: u64,
    pub val: u64,
}

int_enum! {
    /// The calls from PEMux to the kernel
    pub struct Calls : u64 {
        const EXIT           = 0x0;
    }
}

/// The exit call
#[repr(C, packed)]
pub struct Exit {
    pub op: u64,
    pub vpe_sel: u64,
    pub code: u64,
}
