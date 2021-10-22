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

use super::OptionalValue;

/// The VPE id of PEMux
pub const VPE_ID: u64 = 0xFFFF;
/// The VPE id when PEMux is idling
pub const IDLE_ID: u64 = 0xFFFE;

pub type QuotaId = u64;

pub const DEF_QUOTA_ID: QuotaId = 1;

int_enum! {
    /// The sidecalls from the kernel to PEMux
    pub struct Sidecalls : u64 {
        const VPE_INIT       = 0x0;
        const VPE_CTRL       = 0x1;
        const MAP            = 0x2;
        const TRANSLATE      = 0x3;
        const REM_MSGS       = 0x4;
        const EP_INVAL       = 0x5;
        const DERIVE_QUOTA   = 0x6;
        const GET_QUOTA      = 0x7;
        const SET_QUOTA      = 0x8;
        const REMOVE_QUOTAS  = 0x9;
        const RESET_STATS    = 0xA;
    }
}

int_enum! {
    /// The operations for the `vpe_ctrl` sidecall
    pub struct VPEOp : u64 {
        const START = 0x0;
        const STOP  = 0x1;
    }
}

/// The VPE init sidecall
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct VPEInit {
    pub op: u64,
    pub vpe_sel: u64,
    pub time_quota: u64,
    pub pt_quota: u64,
    pub eps_start: u64,
}

/// The VPE control sidecall
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct VPECtrl {
    pub op: u64,
    pub vpe_sel: u64,
    pub vpe_op: u64,
}

/// The map sidecall
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Map {
    pub op: u64,
    pub vpe_sel: u64,
    pub virt: u64,
    pub global: u64,
    pub pages: u64,
    pub perm: u64,
}

/// The translate sidecall
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Translate {
    pub op: u64,
    pub vpe_sel: u64,
    pub virt: u64,
    pub perm: u64,
}

/// The remove messages sidecall
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct RemMsgs {
    pub op: u64,
    pub vpe_sel: u64,
    pub unread_mask: u64,
}

/// The EP invalidation sidecall
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct EpInval {
    pub op: u64,
    pub vpe_sel: u64,
    pub ep: u64,
}

/// The derive quota sidecall
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct DeriveQuota {
    pub op: u64,
    pub parent_time: u64,
    pub parent_pts: u64,
    pub time: OptionalValue,
    pub pts: OptionalValue,
}

/// The get quota sidecall
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct GetQuota {
    pub op: u64,
    pub time: u64,
    pub pts: u64,
}

/// The set quota sidecall
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct SetQuota {
    pub op: u64,
    pub id: u64,
    pub time: u64,
    pub pts: u64,
}

/// The remove quotas sidecall
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct RemoveQuotas {
    pub op: u64,
    pub time: OptionalValue,
    pub pts: OptionalValue,
}

/// The reset stats sidecall
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ResetStats {
    pub op: u64,
}

/// The sidecall response
#[repr(C)]
pub struct Response {
    pub error: u64,
    pub val1: u64,
    pub val2: u64,
}

int_enum! {
    /// The calls from PEMux to the kernel
    pub struct Calls : u64 {
        const EXIT           = 0x0;
    }
}

/// The exit call
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Exit {
    pub op: u64,
    pub vpe_sel: u64,
    pub code: u64,
}
