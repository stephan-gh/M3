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

//! The upcall interface

int_enum! {
    /// The upcalls
    pub struct Operation : u64 {
        /// completions of the derive-srv syscall
        const DERIVE_SRV        = 0;

        /// waits for activity exits
        const ACT_WAIT          = 1;
    }
}

/// The default upcall, containing the opcode and event
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct DefaultUpcall {
    pub opcode: u64,
    pub event: u64,
}

/// The activity-wait upcall that is sent upon a activity-exit
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct ActivityWait {
    pub def: DefaultUpcall,
    pub error: u64,
    pub act_sel: u64,
    pub exitcode: u64,
}

/// The derive-srv upcall that is sent upon completion
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct DeriveSrv {
    pub def: DefaultUpcall,
    pub error: u64,
}
