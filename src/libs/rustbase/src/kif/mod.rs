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
mod perm;
mod pedesc;

pub mod boot;
pub mod service;
pub mod syscalls;
pub mod upcalls;

pub use self::perm::*;
pub use self::pedesc::*;
pub use self::cap::*;

use dtu;

/// Represents an invalid capability selector
pub const INVALID_SEL: CapSel = 0xFFFF;

/// The first selector for the endpoint capabilities (0 and 1 are reserved for VPE cap and mem cap)
pub const FIRST_EP_SEL: CapSel    = 2;

/// The first free selector
pub const FIRST_FREE_SEL: CapSel  = FIRST_EP_SEL + (dtu::EP_COUNT - dtu::FIRST_FREE_EP) as CapSel;
