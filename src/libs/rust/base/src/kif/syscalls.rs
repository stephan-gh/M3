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

//! The system call interface

use super::cap::CapSel;
use arch::tcu::Label;
use core::mem::MaybeUninit;

/// The maximum message length that can be used
pub const MAX_MSG_SIZE: usize = 440;

/// The maximum size of strings in system calls
pub const MAX_STR_SIZE: usize = 64;

/// The maximum number of arguments for the exchange syscalls
pub const MAX_EXCHG_ARGS: usize = 8;

/// The maximum number of VPEs one can wait for
pub const MAX_WAIT_VPES: usize = 48;

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
        const CREATE_VPE        = 6;
        const CREATE_SEM        = 7;
        const ALLOC_EP          = 8;

        // capability operations
        const ACTIVATE          = 9;
        const VPE_CTRL          = 10;
        const VPE_WAIT          = 11;
        const DERIVE_MEM        = 12;
        const DERIVE_KMEM       = 13;
        const DERIVE_PE         = 14;
        const DERIVE_SRV        = 15;
        const GET_SESS          = 16;
        const KMEM_QUOTA        = 17;
        const PE_QUOTA          = 18;
        const SEM_CTRL          = 19;

        // capability exchange
        const DELEGATE          = 20;
        const OBTAIN            = 21;
        const EXCHANGE          = 22;
        const REVOKE            = 23;

        // misc
        const NOOP              = 24;
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
    /// Creates a new `CreateSrv` message with given content
    pub fn new(dst: CapSel, rgate: CapSel, name: &str, creator: Label) -> Self {
        #[allow(clippy::uninit_assumed_init)]
        let mut msg = Self {
            opcode: Operation::CREATE_SRV.val,
            dst_sel: u64::from(dst),
            rgate_sel: u64::from(rgate),
            creator: u64::from(creator),
            namelen: name.len() as u64,
            // safety: will be initialized below
            name: unsafe { MaybeUninit::uninit().assume_init() },
        };

        // copy name
        for (a, c) in msg.name.iter_mut().zip(name.bytes()) {
            *a = c as u8;
        }
        msg
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
    pub vpe_sel: u64,
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
    pub vpe_sel: u64,
    pub mgate_sel: u64,
    pub first: u64,
    pub pages: u64,
    pub perms: u64,
}

/// The create VPE request message
#[repr(C)]
pub struct CreateVPE {
    pub opcode: u64,
    pub dst_sel: u64,
    pub pg_sg_sel: u64,
    pub pg_rg_sel: u64,
    pub pe_sel: u64,
    pub kmem_sel: u64,
    pub namelen: u64,
    pub name: [u8; MAX_STR_SIZE],
}

impl CreateVPE {
    /// Creates a new `CreateVPE` message with given content
    pub fn new(
        dst: CapSel,
        pg_sg: CapSel,
        pg_rg: CapSel,
        name: &str,
        pe: CapSel,
        kmem: CapSel,
    ) -> Self {
        #[allow(clippy::uninit_assumed_init)]
        let mut msg = Self {
            opcode: Operation::CREATE_VPE.val,
            dst_sel: u64::from(dst),
            pg_sg_sel: u64::from(pg_sg),
            pg_rg_sel: u64::from(pg_rg),
            pe_sel: u64::from(pe),
            kmem_sel: u64::from(kmem),
            namelen: name.len() as u64,
            // safety: will be initialized below
            name: unsafe { MaybeUninit::uninit().assume_init() },
        };

        // copy name
        for (a, c) in msg.name.iter_mut().zip(name.bytes()) {
            *a = c as u8;
        }
        msg
    }
}

/// The create VPE reply message
#[repr(C)]
pub struct CreateVPEReply {
    pub error: u64,
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
    pub vpe_sel: u64,
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

int_enum! {
    /// The operations for the `vpe_ctrl` system call
    pub struct VPEOp : u64 {
        const INIT  = 0x0;
        const START = 0x1;
        const STOP  = 0x2;
    }
}

/// The VPE control request message
#[repr(C)]
pub struct VPECtrl {
    pub opcode: u64,
    pub vpe_sel: u64,
    pub op: u64,
    pub arg: u64,
}

/// The VPE wait request message
#[repr(C)]
pub struct VPEWait {
    pub opcode: u64,
    pub vpe_count: u64,
    pub event: u64,
    pub sels: [u64; MAX_WAIT_VPES],
}

impl VPEWait {
    /// Creates a new `VPEWait` message with given content
    pub fn new(vpes: &[CapSel], event: u64) -> Self {
        #[allow(clippy::uninit_assumed_init)]
        let mut msg = Self {
            opcode: Operation::VPE_WAIT.val,
            event,
            vpe_count: vpes.len() as u64,
            // safety: will be initialized below
            sels: unsafe { MaybeUninit::uninit().assume_init() },
        };
        for (i, sel) in vpes.iter().enumerate() {
            msg.sels[i] = u64::from(*sel);
        }
        msg
    }
}

/// The VPE wait reply message
#[repr(C)]
pub struct VPEWaitReply {
    pub error: u64,
    pub vpe_sel: u64,
    pub exitcode: u64,
}

/// The derive memory request message
#[repr(C)]
pub struct DeriveMem {
    pub opcode: u64,
    pub vpe_sel: u64,
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

/// The derive PE request message
#[repr(C)]
pub struct DerivePE {
    pub opcode: u64,
    pub pe_sel: u64,
    pub dst_sel: u64,
    pub eps: u64,
}

/// The derive service request message
#[repr(C)]
pub struct DeriveSrv {
    pub opcode: u64,
    pub dst_sel: u64,
    pub srv_sel: u64,
    pub sessions: u64,
}

/// The get sesion message
#[repr(C)]
pub struct GetSession {
    pub opcode: u64,
    pub dst_sel: u64,
    pub srv_sel: u64,
    pub vpe_sel: u64,
    pub sid: u64,
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
    pub amount: u64,
}

/// The PE quota request message
#[repr(C)]
pub struct PEQuota {
    pub opcode: u64,
    pub pe_sel: u64,
}

/// The PE quota reply message
#[repr(C)]
pub struct PEQuotaReply {
    pub error: u64,
    pub amount: u64,
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
    pub vpe_sel: u64,
    pub own_caps: [u64; 2],
    pub other_sel: u64,
    pub obtain: u64,
}

/// The delegate/obtain request message
#[repr(C)]
pub struct ExchangeSess {
    pub opcode: u64,
    pub vpe_sel: u64,
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
    pub vpe_sel: u64,
    pub caps: [u64; 2],
    pub own: u64,
}

/// The noop request message
#[repr(C)]
pub struct Noop {
    pub opcode: u64,
}
