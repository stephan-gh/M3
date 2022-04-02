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

//! The system call interface

use crate::kif::CapSel;
use crate::mem::{MaybeUninit, MsgBuf};
use crate::tcu::Label;

use super::OptionalValue;

/// The maximum size of strings in system calls
pub const MAX_STR_SIZE: usize = 64;

/// The maximum number of arguments for the exchange syscalls
pub const MAX_EXCHG_ARGS: usize = 8;

/// The maximum number of activities one can wait for
pub const MAX_WAIT_ACTS: usize = 48;

int_enum! {
    /// The system calls
    pub struct Operation : u64 {
        // capability creations
        const CREATE_SRV        = 0;
        const CREATE_SESS       = 1;
        const CREATE_MGATE      = 2;
        const CREATE_RGATE      = 3;
        const CREATE_SGATE      = 4;
        const CREATE_MAP        = 5;
        const CREATE_ACT        = 6;
        const CREATE_SEM        = 7;
        const ALLOC_EP          = 8;

        // capability operations
        const ACTIVATE          = 9;
        const SET_PMP           = 10;
        const ACT_CTRL          = 11;
        const ACT_WAIT          = 12;
        const DERIVE_MEM        = 13;
        const DERIVE_KMEM       = 14;
        const DERIVE_TILE       = 15;
        const DERIVE_SRV        = 16;
        const GET_SESS          = 17;
        const MGATE_REGION      = 18;
        const KMEM_QUOTA        = 19;
        const TILE_QUOTA        = 20;
        const TILE_SET_QUOTA    = 21;
        const SEM_CTRL          = 22;

        // capability exchange
        const DELEGATE          = 23;
        const OBTAIN            = 24;
        const EXCHANGE          = 25;
        const REVOKE            = 26;

        // misc
        const RESET_STATS       = 27;
        const NOOP              = 28;
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ExchangeArgs {
    pub bytes: u64,
    pub data: [u64; 8],
}

impl Default for ExchangeArgs {
    fn default() -> Self {
        #[allow(clippy::uninit_assumed_init)]
        ExchangeArgs {
            bytes: 0,
            // safety: we will initialize the values between 0 and count-1
            data: unsafe { MaybeUninit::uninit().assume_init() },
        }
    }
}

/// The create service request message
#[repr(C)]
pub struct CreateSrv {
    pub opcode: u64,
    pub dst_sel: u64,
    pub rgate_sel: u64,
    pub creator: u64,
    pub namelen: u64,
    pub name: [u8; MAX_STR_SIZE],
}

impl CreateSrv {
    /// Stores a `CreateSrv` message into the given message buffer
    pub fn fill_msgbuf(buf: &mut MsgBuf, dst: CapSel, rgate: CapSel, name: &str, creator: Label) {
        #[allow(clippy::uninit_assumed_init, clippy::useless_conversion)]
        let msg = buf.set(Self {
            opcode: Operation::CREATE_SRV.val,
            dst_sel: dst,
            rgate_sel: rgate,
            creator: u64::from(creator),
            namelen: name.len() as u64,
            // safety: will be initialized below
            name: unsafe { MaybeUninit::uninit().assume_init() },
        });
        // copy name
        for (a, c) in msg.name.iter_mut().zip(name.bytes()) {
            *a = c as u8;
        }
    }
}

/// The create session request message
#[repr(C)]
pub struct CreateSess {
    pub opcode: u64,
    pub dst_sel: u64,
    pub srv_sel: u64,
    pub creator: u64,
    pub ident: u64,
    pub auto_close: u64,
}

/// The create memory gate request message
#[repr(C)]
pub struct CreateMGate {
    pub opcode: u64,
    pub dst_sel: u64,
    pub act_sel: u64,
    pub addr: u64,
    pub size: u64,
    pub perms: u64,
}

/// The create receive gate request message
#[repr(C)]
pub struct CreateRGate {
    pub opcode: u64,
    pub dst_sel: u64,
    pub order: u64,
    pub msgorder: u64,
}

/// The create send gate request message
#[repr(C)]
pub struct CreateSGate {
    pub opcode: u64,
    pub dst_sel: u64,
    pub rgate_sel: u64,
    pub label: u64,
    pub credits: u64,
}

/// The create mapping request message
#[repr(C)]
pub struct CreateMap {
    pub opcode: u64,
    pub dst_sel: u64,
    pub act_sel: u64,
    pub mgate_sel: u64,
    pub first: u64,
    pub pages: u64,
    pub perms: u64,
}

/// The create activity request message
#[repr(C)]
pub struct CreateActivity {
    pub opcode: u64,
    pub dst_sel: u64,
    pub tile_sel: u64,
    pub kmem_sel: u64,
    pub namelen: u64,
    pub name: [u8; MAX_STR_SIZE],
}

impl CreateActivity {
    /// Stores a new `CreateActivity` message into the given message buffer
    pub fn fill_msgbuf(buf: &mut MsgBuf, dst: CapSel, name: &str, tile: CapSel, kmem: CapSel) {
        #[allow(clippy::uninit_assumed_init)]
        let msg = buf.set(Self {
            opcode: Operation::CREATE_ACT.val,
            dst_sel: dst,
            tile_sel: tile,
            kmem_sel: kmem,
            namelen: name.len() as u64,
            // safety: will be initialized below
            name: unsafe { MaybeUninit::uninit().assume_init() },
        });
        // copy name
        for (a, c) in msg.name.iter_mut().zip(name.bytes()) {
            *a = c as u8;
        }
    }
}

/// The create activity reply message
#[repr(C)]
pub struct CreateActivityReply {
    pub error: u64,
    pub id: u64,
    pub eps_start: u64,
}

/// The create semaphore request message
#[repr(C)]
pub struct CreateSem {
    pub opcode: u64,
    pub dst_sel: u64,
    pub value: u64,
}

/// The alloc endpoints request message
#[repr(C)]
pub struct AllocEP {
    pub opcode: u64,
    pub dst_sel: u64,
    pub act_sel: u64,
    pub epid: u64,
    pub replies: u64,
}

/// The alloc endpoints reply message
#[repr(C)]
pub struct AllocEPReply {
    pub error: u64,
    pub ep: u64,
}

/// The activate request message
#[repr(C)]
pub struct Activate {
    pub opcode: u64,
    pub ep_sel: u64,
    pub gate_sel: u64,
    pub rbuf_mem: u64,
    pub rbuf_off: u64,
}

/// The set physical memory protection EP message
#[repr(C)]
pub struct SetPMP {
    pub opcode: u64,
    pub tile_sel: u64,
    pub mgate_sel: u64,
    pub epid: u64,
}

int_enum! {
    /// The operations for the `act_ctrl` system call
    pub struct ActivityOp : u64 {
        const INIT  = 0x0;
        const START = 0x1;
        const STOP  = 0x2;
    }
}

/// The activity control request message
#[repr(C)]
pub struct ActivityCtrl {
    pub opcode: u64,
    pub act_sel: u64,
    pub op: u64,
    pub arg: u64,
}

/// The activity wait request message
#[repr(C)]
pub struct ActivityWait {
    pub opcode: u64,
    pub act_count: u64,
    pub event: u64,
    pub sels: [u64; MAX_WAIT_ACTS],
}

impl ActivityWait {
    /// Stores a new `ActivityWait` message into given message buffer
    pub fn fill_msgbuf(buf: &mut MsgBuf, acts: &[CapSel], event: u64) {
        #[allow(clippy::uninit_assumed_init)]
        let msg = buf.set(Self {
            opcode: Operation::ACT_WAIT.val,
            event,
            act_count: acts.len() as u64,
            // safety: will be initialized below
            sels: unsafe { MaybeUninit::uninit().assume_init() },
        });
        for (i, sel) in acts.iter().enumerate() {
            msg.sels[i] = *sel;
        }
    }
}

/// The activity wait reply message
#[repr(C)]
pub struct ActivityWaitReply {
    pub error: u64,
    pub act_sel: u64,
    pub exitcode: u64,
}

/// The derive memory request message
#[repr(C)]
pub struct DeriveMem {
    pub opcode: u64,
    pub act_sel: u64,
    pub dst_sel: u64,
    pub src_sel: u64,
    pub offset: u64,
    pub size: u64,
    pub perms: u64,
}

/// The derive kernel memory request message
#[repr(C)]
pub struct DeriveKMem {
    pub opcode: u64,
    pub kmem_sel: u64,
    pub dst_sel: u64,
    pub quota: u64,
}

/// The derive tile request message
#[repr(C)]
pub struct DeriveTile {
    pub opcode: u64,
    pub tile_sel: u64,
    pub dst_sel: u64,
    pub eps: OptionalValue,
    pub time: OptionalValue,
    pub pts: OptionalValue,
}

/// The derive service request message
#[repr(C)]
pub struct DeriveSrv {
    pub opcode: u64,
    pub dst_sel: u64,
    pub srv_sel: u64,
    pub sessions: u64,
    pub event: u64,
}

/// The get sesion message
#[repr(C)]
pub struct GetSession {
    pub opcode: u64,
    pub dst_sel: u64,
    pub srv_sel: u64,
    pub act_sel: u64,
    pub sid: u64,
}

/// The memory gate region request message
#[repr(C)]
pub struct MGateRegion {
    pub opcode: u64,
    pub mgate_sel: u64,
}

/// The kernel gate region reply message
#[repr(C)]
pub struct MGateRegionReply {
    pub error: u64,
    pub global: u64,
    pub size: u64,
}

/// The kernel memory quota request message
#[repr(C)]
pub struct KMemQuota {
    pub opcode: u64,
    pub kmem_sel: u64,
}

/// The kernel memory quota reply message
#[repr(C)]
pub struct KMemQuotaReply {
    pub error: u64,
    pub id: u64,
    pub total: u64,
    pub left: u64,
}

/// The tile quota request message
#[repr(C)]
pub struct TileQuota {
    pub opcode: u64,
    pub tile_sel: u64,
}

/// The tile quota reply message
#[repr(C)]
pub struct TileQuotaReply {
    pub error: u64,
    pub eps_id: u64,
    pub eps_total: u64,
    pub eps_left: u64,
    pub time_id: u64,
    pub time_total: u64,
    pub time_left: u64,
    pub pts_id: u64,
    pub pts_total: u64,
    pub pts_left: u64,
}

/// The tile set quota request message
#[repr(C)]
pub struct TileSetQuota {
    pub opcode: u64,
    pub tile_sel: u64,
    pub time: u64,
    pub pts: u64,
}

int_enum! {
    /// The operations for the `sem_ctrl` system call
    pub struct SemOp : u64 {
        const UP   = 0x0;
        const DOWN = 0x1;
    }
}

/// The semaphore control request message
#[repr(C)]
pub struct SemCtrl {
    pub opcode: u64,
    pub sem_sel: u64,
    pub op: u64,
}

/// The exchange request message
#[repr(C)]
pub struct Exchange {
    pub opcode: u64,
    pub act_sel: u64,
    pub own_caps: [u64; 2],
    pub other_sel: u64,
    pub obtain: u64,
}

/// The delegate/obtain request message
#[repr(C)]
pub struct ExchangeSess {
    pub opcode: u64,
    pub act_sel: u64,
    pub sess_sel: u64,
    pub caps: [u64; 2],
    pub args: ExchangeArgs,
}

/// The delegate/obtain reply message
#[repr(C)]
pub struct ExchangeSessReply {
    pub error: u64,
    pub args: ExchangeArgs,
}

/// The revoke request message
#[repr(C)]
pub struct Revoke {
    pub opcode: u64,
    pub act_sel: u64,
    pub caps: [u64; 2],
    pub own: u64,
}

/// The reset stats request message
#[repr(C)]
pub struct ResetStats {
    pub opcode: u64,
}

/// The noop request message
#[repr(C)]
pub struct Noop {
    pub opcode: u64,
}
