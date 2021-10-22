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

//! Contains the kernel interface definitions

mod cap;
mod pedesc;
mod perm;

pub mod boot;
pub mod pemux;
pub mod service;
pub mod syscalls;
pub mod upcalls;

pub use self::cap::*;
pub use self::pedesc::*;
pub use self::perm::*;

use num_traits::PrimInt;

use crate::tcu;

/// Represents an invalid capability selector
pub const INVALID_SEL: CapSel = 0xFFFF;

/// Represents unlimited credits for a SendGate
pub const UNLIM_CREDITS: u32 = tcu::UNLIM_CREDITS;

/// The selector for the own PE capability
pub const SEL_PE: CapSel = 0;
/// The selector for the own kernel memory capability
pub const SEL_KMEM: CapSel = 1;
/// The selector for the own VPE
pub const SEL_VPE: CapSel = 2;

/// The first free selector
pub const FIRST_FREE_SEL: CapSel = SEL_VPE + 1;

/// The default request message that only contains the opcode
#[repr(C)]
pub struct DefaultRequest {
    pub opcode: u64,
}

/// The default reply message that only contains the error code
#[repr(C)]
pub struct DefaultReply {
    pub error: u64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct OptionalValue(u64);

impl OptionalValue {
    pub fn new<T: PrimInt>(val: Option<T>) -> Self
    where
        u64: From<T>,
    {
        match val {
            Some(v) => Self(v.into()),
            None => Self(!0),
        }
    }

    pub fn get<T: PrimInt>(&self) -> Option<T> {
        match self.0 {
            v if v == !0 => None,
            v => T::from(v),
        }
    }
}
