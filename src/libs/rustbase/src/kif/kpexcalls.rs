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

int_enum! {
    /// The kpex calls
    pub struct Operation : u64 {
        const ACTIVATE      = 0x0;
    }
}

/// The activate request message
#[repr(C, packed)]
pub struct Activate {
    pub op: u64,
    pub vpe_sel: u64,
    pub gate_sel: u64,
    pub ep: u64,
    pub addr: u64,
}
