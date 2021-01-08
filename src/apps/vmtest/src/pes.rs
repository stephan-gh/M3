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

use base::envdata;
use base::tcu::PEId;

static PE_IDS: [[PEId; 9]; 2] = [
    // platform = gem5
    [0, 1, 2, 3, 4, 5, 6, 7, 8],
    // platform = hw
    [0x6, 0x25, 0x26, 0x00, 0x01, 0x02, 0x20, 0x21, 0x24],
];

#[allow(dead_code)]
#[repr(usize)]
pub enum PE {
    PE0,
    PE1,
    PE2,
    PE3,
    PE4,
    PE5,
    PE6,
    PE7,
    MEM,
}

impl PE {
    pub fn id(&self) -> PEId {
        // get the index in the enum
        let idx: usize = unsafe { *(self as *const _ as *const usize) };
        PE_IDS[envdata::get().platform as usize][idx]
    }
}
