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

int_enum! {
    pub struct Operation : isize {
        const SEND          = 0x0;
        const REPLY         = 0x1;
        const CALL          = 0x2;

        const FETCH         = 0x3;
        const RECV          = 0x4;
        const ACK           = 0x5;

        const READ          = 0x6;
        const WRITE         = 0x7;

        const SLEEP         = 0x8;
        const EXIT          = 0x9;

        const SWITCH_GATE   = 0xA;
        const REMOVE_GATE   = 0xB;
    }
}
