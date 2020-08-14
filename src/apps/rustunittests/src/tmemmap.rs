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

use m3::errors::Code;
use m3::mem::MemMap;
use m3::test;
use m3::{wv_assert_eq, wv_assert_err, wv_run_test};

pub fn run(t: &mut dyn test::WvTester) {
    wv_run_test!(t, basics);
}

fn basics() {
    let mut m = MemMap::new(0, 0x1000);

    wv_assert_eq!(m.allocate(0x100, 0x10), Ok(0x0));
    wv_assert_eq!(m.allocate(0x100, 0x10), Ok(0x100));
    wv_assert_eq!(m.allocate(0x100, 0x10), Ok(0x200));

    m.free(0x100, 0x100);
    m.free(0x0, 0x100);

    wv_assert_err!(m.allocate(0x1000, 0x10), Code::OutOfMem);
    wv_assert_eq!(m.allocate(0x200, 0x10), Ok(0x0));

    m.free(0x200, 0x100);
    m.free(0x0, 0x200);

    wv_assert_eq!(m.size(), (0x1000, 1));
}
