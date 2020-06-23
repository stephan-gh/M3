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

//! The upcall interface

int_enum! {
    /// The upcalls
    pub struct Operation : u64 {
        /// forwarding of sends, replies, and data transfers
        const FORWARD           = 0;

        /// waits for VPE exits
        const VPEWAIT           = 1;
    }
}

/// The default upcall, containing the opcode and event
#[repr(C)]
#[derive(Copy, Clone)]
pub struct DefaultUpcall {
    pub opcode: u64,
    pub event: u64,
}

/// The forward upcall that is sent upon finished forwardings
#[repr(C)]
#[derive(Copy, Clone)]
pub struct Forward {
    pub def: DefaultUpcall,
    pub error: u64,
}

/// The VPE-wait upcall that is sent upon a VPE-exit
#[repr(C)]
#[derive(Copy, Clone)]
pub struct VPEWait {
    pub def: DefaultUpcall,
    pub error: u64,
    pub vpe_sel: u64,
    pub exitcode: u64,
}
