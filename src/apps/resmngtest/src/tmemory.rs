/*
 * Copyright (C) 2023 Nils Asmussen, Barkhausen Institut
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

use m3::com::MemCap;
use m3::errors::Code;
use m3::kif::Perm;
use m3::mem::GlobAddr;
use m3::rc::Rc;
use m3::tcu::TileId;
use m3::test::WvTester;
use m3::{wv_assert_eq, wv_assert_err, wv_assert_ok, wv_run_test};

use resmng::resources::memory::{MemMod, MemoryManager};

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, mng_basics);
    wv_run_test!(t, mng_multi);
    wv_run_test!(t, mng_pool);
}

fn mng_basics(t: &mut dyn WvTester) {
    let mut mng = MemoryManager::default();
    mng.add(Rc::new(MemMod::new(
        MemCap::new_bind(1),
        GlobAddr::new_with(TileId::new(0, 0), 0x1000),
        0x4000,
        false,
    )));

    wv_assert_eq!(t, mng.capacity(), 0x4000);
    wv_assert_eq!(t, mng.available(), 0x4000);

    {
        let slice = wv_assert_ok!(mng.find_mem(
            GlobAddr::new_with(TileId::new(0, 0), 0x2000),
            0x2000,
            Perm::RW,
        ));
        wv_assert_eq!(
            t,
            slice.addr(),
            GlobAddr::new_with(TileId::new(0, 0), 0x2000)
        );
        wv_assert_eq!(t, slice.capacity(), 0x2000);
        wv_assert_eq!(t, slice.sel(), 1);
    }

    wv_assert_eq!(t, mng.capacity(), 0x4000);
    wv_assert_eq!(t, mng.available(), 0x4000);

    {
        let slice = wv_assert_ok!(mng.alloc_mem(0x3000));
        wv_assert_eq!(
            t,
            slice.addr(),
            GlobAddr::new_with(TileId::new(0, 0), 0x1000)
        );
        wv_assert_eq!(t, slice.capacity(), 0x3000);
        wv_assert_eq!(t, slice.sel(), 1);
    }

    wv_assert_eq!(t, mng.capacity(), 0x4000);
    wv_assert_eq!(t, mng.available(), 0x1000);

    wv_assert_err!(t, mng.alloc_mem(0x2000), Code::NoSpace);
}

fn mng_multi(t: &mut dyn WvTester) {
    let mut mng = MemoryManager::default();
    mng.add(Rc::new(MemMod::new(
        MemCap::new_bind(1),
        GlobAddr::new_with(TileId::new(1, 4), 0x10000),
        0x40000,
        false,
    )));
    mng.add(Rc::new(MemMod::new(
        MemCap::new_bind(2),
        GlobAddr::new_with(TileId::new(1, 5), 0x0),
        0x100000,
        false,
    )));

    wv_assert_eq!(t, mng.capacity(), 0x40000 + 0x100000);
    wv_assert_eq!(t, mng.available(), 0x40000 + 0x100000);

    {
        let slice =
            wv_assert_ok!(mng.find_mem(GlobAddr::new_with(TileId::new(1, 5), 0), 0x3000, Perm::R));
        wv_assert_eq!(t, slice.addr(), GlobAddr::new_with(TileId::new(1, 5), 0x0));
        wv_assert_eq!(t, slice.capacity(), 0x3000);
        wv_assert_eq!(t, slice.sel(), 2);
    }

    {
        let slice = wv_assert_ok!(mng.alloc_mem(0x10000));
        wv_assert_eq!(t, slice.capacity(), 0x10000);
        wv_assert_eq!(t, slice.sel(), 1);
    }

    wv_assert_eq!(t, mng.capacity(), 0x40000 + 0x100000);
    wv_assert_eq!(t, mng.available(), 0x30000 + 0x100000);

    {
        let slice = wv_assert_ok!(mng.alloc_mem(0x80000));
        wv_assert_eq!(t, slice.capacity(), 0x80000);
        wv_assert_eq!(t, slice.sel(), 2);
    }

    wv_assert_eq!(t, mng.capacity(), 0x40000 + 0x100000);
    wv_assert_eq!(t, mng.available(), 0x80000);

    {
        let slice = wv_assert_ok!(mng.alloc_mem(0x80000));
        wv_assert_eq!(t, slice.capacity(), 0x80000);
        wv_assert_eq!(t, slice.sel(), 2);
    }

    wv_assert_eq!(t, mng.capacity(), 0x40000 + 0x100000);
    wv_assert_eq!(t, mng.available(), 0);

    wv_assert_err!(t, mng.alloc_mem(0x1000), Code::NoSpace);
}

fn mng_pool(t: &mut dyn WvTester) {
    let mut mng = MemoryManager::default();
    mng.add(Rc::new(MemMod::new(
        MemCap::new_bind(1),
        GlobAddr::new_with(TileId::new(1, 4), 0x10000),
        0x40000,
        false,
    )));
    mng.add(Rc::new(MemMod::new(
        MemCap::new_bind(2),
        GlobAddr::new_with(TileId::new(1, 5), 0x0),
        0x100000,
        false,
    )));

    let mut pool = wv_assert_ok!(mng.alloc_pool(0x120000));
    wv_assert_eq!(t, pool.capacity(), 0x120000);
    wv_assert_eq!(t, pool.available(), 0x120000);

    let alloc1 = wv_assert_ok!(pool.allocate(0x10000));
    wv_assert_eq!(t, alloc1.slice_id(), 0);
    wv_assert_eq!(t, alloc1.addr(), 0);
    wv_assert_eq!(t, alloc1.size(), 0x10000);
    wv_assert_eq!(t, pool.available(), 0x110000);

    let alloc2 = wv_assert_ok!(pool.allocate(0x20000));
    wv_assert_eq!(t, alloc2.slice_id(), 0);
    wv_assert_eq!(t, alloc2.addr(), 0x10000);
    wv_assert_eq!(t, alloc2.size(), 0x20000);
    wv_assert_eq!(t, pool.available(), 0xF0000);

    let alloc3 = wv_assert_ok!(pool.allocate(0x80000));
    wv_assert_eq!(t, alloc3.slice_id(), 1);
    wv_assert_eq!(t, alloc3.addr(), 0);
    wv_assert_eq!(t, alloc3.size(), 0x80000);
    wv_assert_eq!(t, pool.available(), 0x70000);

    pool.free(alloc2);
    wv_assert_eq!(t, pool.available(), 0x90000);
    pool.free(alloc1);
    wv_assert_eq!(t, pool.available(), 0xA0000);
    pool.free(alloc3);
    wv_assert_eq!(t, pool.available(), 0x120000);
}
