/*
 * Copyright (C) 2020-2022 Nils Asmussen, Barkhausen Institut
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

use m3::com::MemGate;
use m3::kif::Perm;
use m3::mem::VirtAddr;
use m3::test::WvTester;
use m3::tiles::Activity;
use m3::{wv_assert_ok, wv_run_test};

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, large_pages);
}

fn large_pages(_t: &mut dyn WvTester) {
    if let Some(pager) = Activity::own().pager() {
        const VIRT: VirtAddr = VirtAddr::new(0x3000_0000);
        const MEM_SIZE: usize = 6 * 1024 * 1024;
        let mem = wv_assert_ok!(MemGate::new(MEM_SIZE, Perm::RW));
        wv_assert_ok!(pager.map_mem(VIRT, &mem, MEM_SIZE, Perm::RW));

        let ptr = VIRT.as_mut_ptr::<u64>();
        unsafe {
            ptr.write(0);
        }

        wv_assert_ok!(pager.unmap(VIRT));
    }
    else {
        m3::println!("Skipping paging test without pager");
    }
}
