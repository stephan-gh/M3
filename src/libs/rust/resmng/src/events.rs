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

use m3::cell::StaticCell;

pub fn alloc_unique_id() -> u64 {
    static NEXT_ID: StaticCell<u64> = StaticCell::new(0);
    NEXT_ID.set(*NEXT_ID + 1);
    *NEXT_ID
}

pub fn uid_to_event(id: u64) -> thread::Event {
    0x8000_0000_0000_0000 | id
}
