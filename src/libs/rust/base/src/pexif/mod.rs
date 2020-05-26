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

//! Contains the interface between applications and PEMux

int_enum! {
    /// The operations PEMux supports
    pub struct Operation : isize {
        /// Sleep for a given duration or until an event occurs
        const SLEEP         = 0x0;
        /// Exit the application
        const EXIT          = 0x1;
        /// Switch to the next ready VPE
        const YIELD         = 0x2;
        /// Noop operation for testing purposes
        const NOOP          = 0x3;
    }
}
