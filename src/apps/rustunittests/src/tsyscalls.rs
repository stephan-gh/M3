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

use m3::cap::Selector;
use m3::cfg::PAGE_SIZE;
use m3::com::{MemGate, RecvGate};
use m3::cpu;
use m3::errors::{Code, Error};
use m3::goff;
use m3::kif::syscalls::{ActivityOp, SemOp};
use m3::kif::{CapRngDesc, CapType, Perm, INVALID_SEL, SEL_ACT, SEL_KMEM, SEL_TILE};
use m3::math;
use m3::server::{Handler, Server, SessId, SessionContainer};
use m3::session::{ServerSession, M3FS};
use m3::syscalls;
use m3::tcu::{AVAIL_EPS, FIRST_USER_EP, TOTAL_EPS};
use m3::test;
use m3::tiles::{Activity, ActivityArgs, Tile};
use m3::{wv_assert, wv_assert_eq, wv_assert_err, wv_assert_ok, wv_run_test};

pub fn run(t: &mut dyn test::WvTester) {
    wv_run_test!(t, create_srv);
    wv_run_test!(t, create_sess);
    wv_run_test!(t, create_mgate);
    wv_run_test!(t, create_rgate);
    wv_run_test!(t, create_sgate);
    wv_run_test!(t, create_map);
    wv_run_test!(t, create_activity);
    wv_run_test!(t, create_sem);
    wv_run_test!(t, alloc_ep);

    wv_run_test!(t, activate);
    wv_run_test!(t, activity_ctrl);
    wv_run_test!(t, derive_mem);
    wv_run_test!(t, derive_kmem);
    wv_run_test!(t, derive_tile);
    wv_run_test!(t, derive_srv);
    wv_run_test!(t, get_sess);
    wv_run_test!(t, mgate_region);
    wv_run_test!(t, kmem_quota);
    wv_run_test!(t, tile_quota);
    wv_run_test!(t, sem_ctrl);

    wv_run_test!(t, delegate);
    wv_run_test!(t, obtain);
    wv_run_test!(t, exchange);
    wv_run_test!(t, revoke);
}

fn create_srv() {
    let sel = Activity::cur().alloc_sel();
    let mut rgate = wv_assert_ok!(RecvGate::new(10, 10));

    // invalid dest selector
    wv_assert_err!(
        syscalls::create_srv(SEL_ACT, rgate.sel(), "test", 0),
        Code::InvArgs
    );

    // invalid rgate selector
    wv_assert_err!(syscalls::create_srv(sel, SEL_ACT, "test", 0), Code::InvArgs);
    // again, with real rgate, but not activated
    wv_assert_err!(
        syscalls::create_srv(sel, rgate.sel(), "test", 0),
        Code::InvArgs
    );
    wv_assert_ok!(rgate.activate());

    // invalid name
    wv_assert_err!(syscalls::create_srv(sel, rgate.sel(), "", 0), Code::InvArgs);
}

fn create_sgate() {
    let sel = Activity::cur().alloc_sel();
    let rgate = wv_assert_ok!(RecvGate::new(10, 10));

    // invalid dest selector
    wv_assert_err!(
        syscalls::create_sgate(SEL_ACT, rgate.sel(), 0xDEAD_BEEF, 123),
        Code::InvArgs
    );
    // invalid rgate selector
    wv_assert_err!(
        syscalls::create_sgate(sel, SEL_ACT, 0xDEAD_BEEF, 123),
        Code::InvArgs
    );
}

fn create_mgate() {
    if !Activity::cur().tile_desc().has_virtmem() {
        return;
    }

    let sel = Activity::cur().alloc_sel();

    // invalid dest selector
    wv_assert_err!(
        syscalls::create_mgate(SEL_ACT, SEL_ACT, 0, PAGE_SIZE as goff, Perm::R),
        Code::InvArgs
    );
    // invalid activity selector
    wv_assert_err!(
        syscalls::create_mgate(sel, SEL_KMEM, 0, PAGE_SIZE as goff, Perm::R),
        Code::InvArgs
    );
    // unaligned virtual address
    wv_assert_err!(
        syscalls::create_mgate(sel, SEL_ACT, 0xFF, PAGE_SIZE as goff, Perm::R),
        Code::InvArgs
    );
    // unaligned size
    wv_assert_err!(
        syscalls::create_mgate(sel, SEL_ACT, 0, PAGE_SIZE as goff - 1, Perm::R),
        Code::InvArgs
    );
    // size is 0
    wv_assert_err!(
        syscalls::create_mgate(sel, SEL_ACT, 0, 0, Perm::R),
        Code::InvArgs
    );

    if Activity::cur().tile_desc().has_virtmem() {
        // it has to be mapped
        wv_assert_err!(
            syscalls::create_mgate(sel, SEL_ACT, 0, PAGE_SIZE as goff, Perm::R),
            Code::InvArgs
        );
        // and respect the permissions
        let addr = cpu::stack_pointer() as goff;
        let addr = math::round_dn(addr, PAGE_SIZE as goff);
        wv_assert_err!(
            syscalls::create_mgate(sel, SEL_ACT, addr, PAGE_SIZE as goff, Perm::X),
            Code::NoPerm
        );

        // create 4-page mapping
        let virt: goff = 0x3000_0000;
        let mem = wv_assert_ok!(MemGate::new(PAGE_SIZE * 4, Perm::RW));
        wv_assert_ok!(syscalls::create_map(
            (virt / PAGE_SIZE as goff) as Selector,
            Activity::cur().sel(),
            mem.sel(),
            0,
            4,
            Perm::RW
        ));

        // it has to be within bounds
        wv_assert_err!(
            syscalls::create_mgate(sel, SEL_ACT, virt, PAGE_SIZE as goff * 5, Perm::W),
            Code::InvArgs
        );
        wv_assert_err!(
            syscalls::create_mgate(
                sel,
                SEL_ACT,
                virt + PAGE_SIZE as goff,
                PAGE_SIZE as goff * 4,
                Perm::W
            ),
            Code::InvArgs
        );
    }

    // the TCU region is off limits
    #[cfg(not(target_vendor = "host"))]
    wv_assert_err!(
        syscalls::create_mgate(
            sel,
            SEL_ACT,
            m3::tcu::MMIO_ADDR as goff,
            PAGE_SIZE as goff,
            Perm::R
        ),
        Code::InvArgs
    );
}

fn create_rgate() {
    let sel = Activity::cur().alloc_sel();

    // invalid dest selector
    wv_assert_err!(syscalls::create_rgate(SEL_ACT, 10, 10), Code::InvArgs);
    // invalid order
    wv_assert_err!(syscalls::create_rgate(sel, 2000, 10), Code::InvArgs);
    wv_assert_err!(syscalls::create_rgate(sel, !0, 10), Code::InvArgs);
    // invalid msg order
    wv_assert_err!(syscalls::create_rgate(sel, 10, 11), Code::InvArgs);
    wv_assert_err!(syscalls::create_rgate(sel, 10, !0), Code::InvArgs);
    // invalid order and msg order
    wv_assert_err!(syscalls::create_rgate(sel, !0, !0), Code::InvArgs);
}

fn create_sess() {
    let srv = Activity::cur().alloc_sel();
    let mut rgate = wv_assert_ok!(RecvGate::new(10, 10));
    wv_assert_ok!(rgate.activate());
    wv_assert_ok!(syscalls::create_srv(srv, rgate.sel(), "test", 0,));

    let sel = Activity::cur().alloc_sel();

    // invalid dest selector
    wv_assert_err!(
        syscalls::create_sess(SEL_ACT, srv, 0, 0, false),
        Code::InvArgs
    );
    // invalid service selector
    wv_assert_err!(
        syscalls::create_sess(sel, SEL_ACT, 0, 0, false),
        Code::InvArgs
    );

    wv_assert_ok!(syscalls::revoke(
        Activity::cur().sel(),
        CapRngDesc::new(CapType::OBJECT, srv, 1),
        true
    ));
}

#[allow(clippy::cognitive_complexity)]
fn create_map() {
    if !Activity::cur().tile_desc().has_virtmem() {
        return;
    }

    let meminv = wv_assert_ok!(MemGate::new(64, Perm::RW)); // not page-granular
    let mem = wv_assert_ok!(MemGate::new(PAGE_SIZE * 4, Perm::RW));

    // invalid activity selector
    wv_assert_err!(
        syscalls::create_map(0, SEL_KMEM, mem.sel(), 0, 4, Perm::RW),
        Code::InvArgs
    );
    // invalid memgate selector
    wv_assert_err!(
        syscalls::create_map(0, Activity::cur().sel(), SEL_ACT, 0, 4, Perm::RW),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::create_map(0, Activity::cur().sel(), meminv.sel(), 0, 4, Perm::RW),
        Code::InvArgs
    );
    // invalid first page
    wv_assert_err!(
        syscalls::create_map(0, Activity::cur().sel(), mem.sel(), 4, 4, Perm::RW),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::create_map(0, Activity::cur().sel(), mem.sel(), !0, 4, Perm::RW),
        Code::InvArgs
    );
    // invalid page count
    wv_assert_err!(
        syscalls::create_map(0, Activity::cur().sel(), mem.sel(), 0, 5, Perm::RW),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::create_map(0, Activity::cur().sel(), mem.sel(), 3, 2, Perm::RW),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::create_map(0, Activity::cur().sel(), mem.sel(), 4, 0, Perm::RW),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::create_map(0, Activity::cur().sel(), mem.sel(), !0, !0, Perm::RW),
        Code::InvArgs
    );
    // invalid permissions
    wv_assert_err!(
        syscalls::create_map(0, Activity::cur().sel(), mem.sel(), 0, 4, Perm::X),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::create_map(0, Activity::cur().sel(), mem.sel(), 0, 4, Perm::RWX),
        Code::InvArgs
    );
}

#[allow(clippy::cognitive_complexity)]
fn create_activity() {
    let sel = Activity::cur().alloc_sel();
    let kmem = Activity::cur().kmem().sel();

    let tile = wv_assert_ok!(Tile::get("clone|own"));

    // invalid dest selector
    wv_assert_err!(
        syscalls::create_activity(SEL_KMEM, "test", tile.sel(), kmem),
        Code::InvArgs
    );

    // invalid name
    wv_assert_err!(
        syscalls::create_activity(sel, "", tile.sel(), kmem),
        Code::InvArgs
    );

    // invalid kmem
    wv_assert_err!(
        syscalls::create_activity(sel, "test", tile.sel(), INVALID_SEL),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::create_activity(sel, "test", tile.sel(), SEL_ACT),
        Code::InvArgs
    );
}

fn create_sem() {
    let sel = Activity::cur().alloc_sel();

    // invalid selector
    wv_assert_err!(syscalls::create_sem(SEL_ACT, 0), Code::InvArgs);
    wv_assert_ok!(syscalls::create_sem(sel, 1));
    // one down does not block us
    wv_assert_ok!(syscalls::sem_ctrl(sel, SemOp::DOWN));

    wv_assert_ok!(Activity::cur().revoke(CapRngDesc::new(CapType::OBJECT, sel, 1), false));
}

fn alloc_ep() {
    let sel = Activity::cur().alloc_sel();

    // try to use the EP object after the activity we allocated it for is gone
    {
        {
            let tile = wv_assert_ok!(Tile::get("clone"));
            let act = wv_assert_ok!(Activity::new_with(tile, ActivityArgs::new("test")));
            wv_assert_ok!(syscalls::alloc_ep(sel, act.sel(), TOTAL_EPS, 1));
        }

        let mgate = wv_assert_ok!(MemGate::new(0x1000, Perm::RW));
        wv_assert_err!(
            syscalls::activate(sel, mgate.sel(), INVALID_SEL, 0),
            Code::InvArgs
        );
    }

    // invalid dest selector
    wv_assert_err!(
        syscalls::alloc_ep(SEL_ACT, Activity::cur().tile().sel(), TOTAL_EPS, 1),
        Code::InvArgs
    );
    // invalid activity selector
    wv_assert_err!(
        syscalls::alloc_ep(sel, SEL_TILE, TOTAL_EPS, 1),
        Code::InvArgs
    );
    // invalid reply count
    wv_assert_err!(
        syscalls::alloc_ep(sel, Activity::cur().sel(), AVAIL_EPS - 2, !0),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::alloc_ep(sel, Activity::cur().sel(), AVAIL_EPS - 2, TOTAL_EPS as u32),
        Code::InvArgs
    );

    // any EP
    let ep = wv_assert_ok!(syscalls::alloc_ep(sel, Activity::cur().sel(), TOTAL_EPS, 1));
    wv_assert!(ep >= FIRST_USER_EP);
    wv_assert!(ep < TOTAL_EPS);
    wv_assert_ok!(Activity::cur().revoke(CapRngDesc::new(CapType::OBJECT, sel, 1), false));

    // specific EP
    let ep = wv_assert_ok!(syscalls::alloc_ep(
        sel,
        Activity::cur().sel(),
        AVAIL_EPS - 2,
        1
    ));
    wv_assert_eq!(ep, AVAIL_EPS - 2);
    wv_assert_ok!(Activity::cur().revoke(CapRngDesc::new(CapType::OBJECT, sel, 1), false));
}

fn activate() {
    let ep1 = wv_assert_ok!(Activity::cur().epmng_mut().acquire(0));
    let ep2 = wv_assert_ok!(Activity::cur().epmng_mut().acquire(0));
    let sel = Activity::cur().alloc_sel();
    let mgate = wv_assert_ok!(MemGate::new(0x1000, Perm::RW));

    // invalid EP sel
    wv_assert_err!(
        syscalls::activate(SEL_ACT, mgate.sel(), INVALID_SEL, 0),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::activate(sel, mgate.sel(), INVALID_SEL, 0),
        Code::InvArgs
    );
    // invalid mgate sel
    wv_assert_err!(
        syscalls::activate(ep1.sel(), SEL_ACT, INVALID_SEL, 0),
        Code::InvArgs
    );
    // receive buffer specified for MemGate
    wv_assert_err!(
        syscalls::activate(ep1.sel(), mgate.sel(), mgate.sel(), 0),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::activate(ep1.sel(), mgate.sel(), INVALID_SEL, 1),
        Code::InvArgs
    );
    // already activated
    wv_assert_ok!(syscalls::activate(ep1.sel(), mgate.sel(), INVALID_SEL, 0));
    wv_assert_err!(
        syscalls::activate(ep2.sel(), mgate.sel(), INVALID_SEL, 0),
        Code::Exists
    );

    Activity::cur().epmng_mut().release(ep2, true);
    Activity::cur().epmng_mut().release(ep1, true);
}

fn derive_mem() {
    let act = Activity::cur().sel();
    let sel = Activity::cur().alloc_sel();
    let mem = wv_assert_ok!(MemGate::new(0x4000, Perm::RW));

    // invalid dest selector
    wv_assert_err!(
        syscalls::derive_mem(act, SEL_ACT, mem.sel(), 0, 0x1000, Perm::RW),
        Code::InvArgs
    );
    // invalid mem
    wv_assert_err!(
        syscalls::derive_mem(act, sel, SEL_ACT, 0, 0x1000, Perm::RW),
        Code::InvArgs
    );
    // invalid offset
    wv_assert_err!(
        syscalls::derive_mem(act, sel, mem.sel(), 0x4000, 0x1000, Perm::RW),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::derive_mem(act, sel, mem.sel(), !0, 0x1000, Perm::RW),
        Code::InvArgs
    );
    // invalid size
    wv_assert_err!(
        syscalls::derive_mem(act, sel, mem.sel(), 0, 0x4001, Perm::RW),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::derive_mem(act, sel, mem.sel(), 0x2000, 0x2001, Perm::RW),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::derive_mem(act, sel, mem.sel(), 0x2000, 0, Perm::RW),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::derive_mem(act, sel, mem.sel(), 0x4000, 0, Perm::RW),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::derive_mem(act, sel, mem.sel(), !0, !0, Perm::RW),
        Code::InvArgs
    );
    // perms are arbitrary; will be ANDed
}

fn derive_kmem() {
    let sel = Activity::cur().alloc_sel();
    let quota = wv_assert_ok!(Activity::cur().kmem().quota()).left();

    // invalid dest selector
    wv_assert_err!(
        syscalls::derive_kmem(Activity::cur().kmem().sel(), SEL_ACT, quota / 2),
        Code::InvArgs
    );
    // invalid quota
    wv_assert_err!(
        syscalls::derive_kmem(Activity::cur().kmem().sel(), sel, quota + 1),
        Code::NoSpace
    );
    // invalid kmem sel
    wv_assert_err!(
        syscalls::derive_kmem(SEL_ACT, sel, quota + 1),
        Code::InvArgs
    );

    // do that test twice, because we might cause pagefaults during the first test, changing the
    // kernel memory quota (our pager shares the kmem with us).
    for i in 0..=1 {
        let before = wv_assert_ok!(Activity::cur().kmem().quota()).left();
        // transfer memory
        {
            let kmem2 = wv_assert_ok!(Activity::cur().kmem().derive(before / 2));
            let quota2 = wv_assert_ok!(kmem2.quota()).left();
            let nquota = wv_assert_ok!(Activity::cur().kmem().quota()).left();
            wv_assert_eq!(quota2, before / 2);
            // we don't know exactly, because we have paid for the new cap and kobject too
            wv_assert!(nquota <= before / 2);
        }
        // only do the check in the second test where no pagefaults should occur
        if i == 1 {
            let nquota = wv_assert_ok!(Activity::cur().kmem().quota()).left();
            wv_assert_eq!(nquota, before);
        }
    }

    let kmem = wv_assert_ok!(Activity::cur().kmem().derive(quota / 2));
    {
        let tile = wv_assert_ok!(Tile::get("clone"));
        let _act = wv_assert_ok!(Activity::new_with(
            tile,
            ActivityArgs::new("test").kmem(kmem.clone())
        ));
        // activity is still using the kmem
        wv_assert_err!(
            Activity::cur().revoke(CapRngDesc::new(CapType::OBJECT, kmem.sel(), 1), false),
            Code::NotRevocable
        );
    }

    // now we can revoke it
    wv_assert_ok!(Activity::cur().revoke(CapRngDesc::new(CapType::OBJECT, kmem.sel(), 1), false));
}

fn derive_tile() {
    let sel = Activity::cur().alloc_sel();
    let tile = wv_assert_ok!(Tile::get("clone"));
    let oquota = wv_assert_ok!(tile.quota()).0.left();

    // invalid dest selector
    wv_assert_err!(
        syscalls::derive_tile(tile.sel(), SEL_ACT, Some(1), None, None),
        Code::InvArgs
    );
    // invalid ep count
    wv_assert_err!(
        syscalls::derive_tile(tile.sel(), sel, Some(oquota + 1), None, None),
        Code::NoSpace
    );
    // invalid tile sel
    wv_assert_err!(
        syscalls::derive_tile(SEL_ACT, sel, Some(1), None, None),
        Code::InvArgs
    );

    // transfer EPs
    {
        let tile2 = wv_assert_ok!(tile.derive(Some(1), None, None));
        let quota2 = wv_assert_ok!(tile2.quota()).0.left();
        let nquota = wv_assert_ok!(tile.quota()).0.left();
        wv_assert_eq!(quota2, 1);
        wv_assert_eq!(nquota, oquota - 1);
    }
    let nquota = wv_assert_ok!(tile.quota()).0.left();
    wv_assert_eq!(nquota, oquota);

    {
        let _act = wv_assert_ok!(Activity::new(tile.clone(), "test"));
        // activity is still using the Tile
        wv_assert_err!(
            Activity::cur().revoke(CapRngDesc::new(CapType::OBJECT, tile.sel(), 1), false),
            Code::NotRevocable
        );
    }

    // now we can revoke it
    wv_assert_ok!(Activity::cur().revoke(CapRngDesc::new(CapType::OBJECT, tile.sel(), 1), false));
}

struct DummyHandler {
    sessions: SessionContainer<()>,
}

impl Handler<()> for DummyHandler {
    fn sessions(&mut self) -> &mut SessionContainer<()> {
        &mut self.sessions
    }

    fn open(&mut self, _: usize, _: Selector, _: &str) -> Result<(Selector, SessId), Error> {
        Err(Error::new(Code::NotSup))
    }
}

fn derive_srv() {
    let crd = CapRngDesc::new(CapType::OBJECT, Activity::cur().alloc_sels(2), 2);
    let mut hdl = DummyHandler {
        sessions: SessionContainer::new(16),
    };
    let srv = wv_assert_ok!(Server::new_private("test", &mut hdl));

    // invalid service selector
    wv_assert_err!(syscalls::derive_srv(SEL_KMEM, crd, 1, 0), Code::InvArgs);
    // invalid dest selector
    wv_assert_err!(
        syscalls::derive_srv(
            srv.sel(),
            CapRngDesc::new(CapType::OBJECT, SEL_KMEM, 2),
            1,
            0
        ),
        Code::InvArgs
    );
    // invalid session count
    wv_assert_err!(syscalls::derive_srv(srv.sel(), crd, 0, 0), Code::InvArgs);
}

fn get_sess() {
    let sel = Activity::cur().alloc_sel();
    let mut hdl = DummyHandler {
        sessions: SessionContainer::new(16),
    };
    let srv = wv_assert_ok!(Server::new_private("test", &mut hdl));

    let _sess1 = wv_assert_ok!(ServerSession::new(srv.sel(), 0, 0xDEAD_BEEF, false));
    let _sess2 = wv_assert_ok!(ServerSession::new(srv.sel(), 1, 0x1234, false));

    // dummy activity that should receive the session
    let tile = wv_assert_ok!(Tile::get("clone|own"));
    let act = wv_assert_ok!(Activity::new(tile, "test"));

    // invalid service selector
    wv_assert_err!(
        syscalls::get_sess(SEL_KMEM, act.sel(), sel, 0xDEAD_BEEF),
        Code::InvArgs
    );
    // invalid activity selector
    wv_assert_err!(
        syscalls::get_sess(srv.sel(), SEL_KMEM, sel, 0xDEAD_BEEF),
        Code::InvArgs
    );
    // own activity selector
    wv_assert_err!(
        syscalls::get_sess(srv.sel(), Activity::cur().sel(), sel, 0xDEAD_BEEF),
        Code::InvArgs
    );
    // invalid destination selector
    wv_assert_err!(
        syscalls::get_sess(srv.sel(), act.sel(), SEL_KMEM, 0xDEAD_BEEF),
        Code::InvArgs
    );
    // unknown session
    wv_assert_err!(
        syscalls::get_sess(srv.sel(), act.sel(), sel, 0x2222),
        Code::InvArgs
    );
    // not our session
    wv_assert_err!(
        syscalls::get_sess(srv.sel(), act.sel(), sel, 0x1234),
        Code::NoPerm
    );

    // success
    wv_assert_ok!(syscalls::get_sess(srv.sel(), act.sel(), sel, 0xDEAD_BEEF));
}

fn mgate_region() {
    // invalid selector
    wv_assert_err!(syscalls::mgate_region(SEL_ACT), Code::InvArgs);
    wv_assert_err!(
        syscalls::mgate_region(Activity::cur().alloc_sel()),
        Code::InvArgs
    );

    let mgate = wv_assert_ok!(MemGate::new(0x2000, Perm::RW));
    let (_global, size) = wv_assert_ok!(mgate.region());
    wv_assert_eq!(size, 0x2000);
}

fn kmem_quota() {
    // invalid selector
    wv_assert_err!(syscalls::kmem_quota(SEL_ACT), Code::InvArgs);
    wv_assert_err!(
        syscalls::kmem_quota(Activity::cur().alloc_sel()),
        Code::InvArgs
    );
}

fn tile_quota() {
    // invalid selector
    wv_assert_err!(syscalls::tile_quota(SEL_ACT), Code::InvArgs);
    wv_assert_err!(
        syscalls::tile_quota(Activity::cur().alloc_sel()),
        Code::InvArgs
    );
}

fn sem_ctrl() {
    // invalid selector
    wv_assert_err!(syscalls::sem_ctrl(SEL_ACT, SemOp::DOWN), Code::InvArgs);
    wv_assert_err!(
        syscalls::sem_ctrl(Activity::cur().alloc_sel(), SemOp::DOWN),
        Code::InvArgs
    );
}

fn activity_ctrl() {
    wv_assert_err!(
        syscalls::activity_ctrl(SEL_KMEM, ActivityOp::START, 0),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::activity_ctrl(INVALID_SEL, ActivityOp::START, 0),
        Code::InvArgs
    );
    // can't start ourself
    wv_assert_err!(
        syscalls::activity_ctrl(Activity::cur().sel(), ActivityOp::START, 0),
        Code::InvArgs
    );
}

fn exchange() {
    let tile = wv_assert_ok!(Tile::get("clone|own"));
    let mut child = wv_assert_ok!(Activity::new(tile, "test"));
    let csel = child.alloc_sel();

    let sel = Activity::cur().alloc_sel();
    let unused = CapRngDesc::new(CapType::OBJECT, sel, 1);
    let used = CapRngDesc::new(CapType::OBJECT, 0, 1);

    // invalid activity sel
    wv_assert_err!(
        syscalls::exchange(SEL_KMEM, used, csel, false),
        Code::InvArgs
    );
    // invalid own caps (source caps can be invalid)
    wv_assert_err!(
        syscalls::exchange(Activity::cur().sel(), used, unused.start(), true),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::exchange(child.sel(), used, 0, true),
        Code::InvArgs
    );
    // invalid other caps
    wv_assert_err!(
        syscalls::exchange(Activity::cur().sel(), used, 0, false),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::exchange(child.sel(), used, 0, false),
        Code::InvArgs
    );
}

fn delegate() {
    let m3fs = wv_assert_ok!(M3FS::new(1, "m3fs-clone"));
    let m3fs = m3fs.borrow();
    let sess = m3fs.as_any().downcast_ref::<M3FS>().unwrap().sess();
    let crd = CapRngDesc::new(CapType::OBJECT, SEL_ACT, 1);

    // invalid activity selector
    wv_assert_err!(
        syscalls::delegate(SEL_KMEM, sess.sel(), crd, |_| {}, |_| Ok(())),
        Code::InvArgs
    );
    // invalid sess selector
    wv_assert_err!(
        syscalls::delegate(Activity::cur().sel(), SEL_ACT, crd, |_| {}, |_| Ok(())),
        Code::InvArgs
    );
    // CRD can be anything (depends on server)
}

fn obtain() {
    let m3fs = wv_assert_ok!(M3FS::new(1, "m3fs-clone"));
    let m3fs = m3fs.borrow();
    let sess = m3fs.as_any().downcast_ref::<M3FS>().unwrap().sess();
    let sel = Activity::cur().alloc_sel();
    let crd = CapRngDesc::new(CapType::OBJECT, sel, 1);
    let inval = CapRngDesc::new(CapType::OBJECT, SEL_ACT, 1);

    // invalid activity selector
    wv_assert_err!(
        syscalls::obtain(SEL_KMEM, sess.sel(), crd, |_| {}, |_| Ok(())),
        Code::InvArgs
    );
    // invalid sess selector
    wv_assert_err!(
        syscalls::obtain(Activity::cur().sel(), SEL_ACT, crd, |_| {}, |_| Ok(())),
        Code::InvArgs
    );
    // invalid CRD
    wv_assert_err!(
        syscalls::obtain(Activity::cur().sel(), sess.sel(), inval, |_| {}, |_| Ok(())),
        Code::InvArgs
    );
}

fn revoke() {
    let crd_tile = CapRngDesc::new(CapType::OBJECT, SEL_TILE, 1);
    let crd_act = CapRngDesc::new(CapType::OBJECT, SEL_ACT, 1);
    let crd_mem = CapRngDesc::new(CapType::OBJECT, SEL_KMEM, 1);

    // invalid activity selector
    wv_assert_err!(syscalls::revoke(SEL_KMEM, crd_act, true), Code::InvArgs);
    // can't revoke Tile, activity, or mem cap
    wv_assert_err!(
        syscalls::revoke(Activity::cur().sel(), crd_tile, true),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::revoke(Activity::cur().sel(), crd_act, true),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::revoke(Activity::cur().sel(), crd_mem, true),
        Code::InvArgs
    );
}
