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

use crate::errors::Code;
use crate::kif::CapSel;
use crate::serialize::{Deserialize, Serialize};

int_enum! {
    /// The upcalls
    pub struct Operation : u64 {
        /// completions of the derive-srv syscall
        const DERIVE_SRV        = 0;

        /// waits for activity exits
        const ACT_WAIT          = 1;
    }
}

/// The activity-wait upcall that is sent upon a activity-exit
#[derive(Debug, Serialize, Deserialize)]
#[repr(C)]
pub struct ActivityWait {
    pub event: u64,
    pub error: Code,
    pub act_sel: CapSel,
    pub exitcode: Code,
}

/// The derive-srv upcall that is sent upon completion
#[derive(Debug, Serialize, Deserialize)]
#[repr(C)]
pub struct DeriveSrv {
    pub event: u64,
    pub error: Code,
}
