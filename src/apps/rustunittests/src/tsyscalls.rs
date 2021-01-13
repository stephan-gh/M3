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

use m3::cap::Selector;
use m3::cfg::PAGE_SIZE;
use m3::com::{MemGate, RecvGate, SendGate};
use m3::cpu;
use m3::errors::{Code, Error};
use m3::goff;
use m3::kif::syscalls::{SemOp, VPEOp};
use m3::kif::{CapRngDesc, CapType, Perm, INVALID_SEL, SEL_KMEM, SEL_PE, SEL_VPE};
use m3::math;
use m3::pes::{VPEArgs, PE, VPE};
use m3::server::{Handler, Server, SessId, SessionContainer};
use m3::session::{ServerSession, M3FS};
use m3::syscalls;
use m3::tcu::{AVAIL_EPS, FIRST_USER_EP, TOTAL_EPS};
use m3::test;
use m3::{wv_assert, wv_assert_eq, wv_assert_err, wv_assert_ok, wv_run_test};

pub fn run(t: &mut dyn test::WvTester) {
    wv_run_test!(t, create_srv);
    wv_run_test!(t, create_sess);
    wv_run_test!(t, create_mgate);
    wv_run_test!(t, create_rgate);
    wv_run_test!(t, create_sgate);
    wv_run_test!(t, create_map);
    wv_run_test!(t, create_vpe);
    wv_run_test!(t, create_sem);
    wv_run_test!(t, alloc_ep);

    wv_run_test!(t, activate);
    wv_run_test!(t, vpe_ctrl);
    wv_run_test!(t, vpe_wait);
    wv_run_test!(t, derive_mem);
    wv_run_test!(t, derive_kmem);
    wv_run_test!(t, derive_pe);
    wv_run_test!(t, derive_srv);
    wv_run_test!(t, get_sess);
    wv_run_test!(t, kmem_quota);
    wv_run_test!(t, pe_quota);
    wv_run_test!(t, sem_ctrl);

    wv_run_test!(t, delegate);
    wv_run_test!(t, obtain);
    wv_run_test!(t, exchange);
    wv_run_test!(t, revoke);
}

fn create_srv() {
    let sel = VPE::cur().alloc_sel();
    let mut rgate = wv_assert_ok!(RecvGate::new(10, 10));

    // invalid dest selector
    wv_assert_err!(
        syscalls::create_srv(SEL_VPE, rgate.sel(), "test", 0),
        Code::InvArgs
    );

    // invalid rgate selector
    wv_assert_err!(syscalls::create_srv(sel, SEL_VPE, "test", 0), Code::InvArgs);
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
    let sel = VPE::cur().alloc_sel();
    let rgate = wv_assert_ok!(RecvGate::new(10, 10));

    // invalid dest selector
    wv_assert_err!(
        syscalls::create_sgate(SEL_VPE, rgate.sel(), 0xDEAD_BEEF, 123),
        Code::InvArgs
    );
    // invalid rgate selector
    wv_assert_err!(
        syscalls::create_sgate(sel, SEL_VPE, 0xDEAD_BEEF, 123),
        Code::InvArgs
    );
}

fn create_mgate() {
    if !VPE::cur().pe_desc().has_virtmem() {
        return;
    }

    let sel = VPE::cur().alloc_sel();

    // invalid dest selector
    wv_assert_err!(
        syscalls::create_mgate(SEL_VPE, SEL_VPE, 0, PAGE_SIZE as goff, Perm::R),
        Code::InvArgs
    );
    // invalid VPE selector
    wv_assert_err!(
        syscalls::create_mgate(sel, SEL_KMEM, 0, PAGE_SIZE as goff, Perm::R),
        Code::InvArgs
    );
    // unaligned virtual address
    wv_assert_err!(
        syscalls::create_mgate(sel, SEL_VPE, 0xFF, PAGE_SIZE as goff, Perm::R),
        Code::InvArgs
    );
    // unaligned size
    wv_assert_err!(
        syscalls::create_mgate(sel, SEL_VPE, 0, PAGE_SIZE as goff - 1, Perm::R),
        Code::InvArgs
    );
    // size is 0
    wv_assert_err!(
        syscalls::create_mgate(sel, SEL_VPE, 0, 0, Perm::R),
        Code::InvArgs
    );

    if VPE::cur().pe_desc().has_virtmem() {
        // it has to be mapped
        wv_assert_err!(
            syscalls::create_mgate(sel, SEL_VPE, 0, PAGE_SIZE as goff, Perm::R),
            Code::InvArgs
        );
        // and respect the permissions
        let addr = cpu::stack_pointer() as goff;
        let addr = math::round_dn(addr, PAGE_SIZE as goff);
        wv_assert_err!(
            syscalls::create_mgate(sel, SEL_VPE, addr, PAGE_SIZE as goff, Perm::X),
            Code::NoPerm
        );

        // create 4-page mapping
        let virt: goff = 0x3000_0000;
        let mem = wv_assert_ok!(MemGate::new(PAGE_SIZE * 4, Perm::RW));
        wv_assert_ok!(syscalls::create_map(
            (virt / PAGE_SIZE as goff) as Selector,
            VPE::cur().sel(),
            mem.sel(),
            0,
            4,
            Perm::RW
        ));

        // it has to be within bounds
        wv_assert_err!(
            syscalls::create_mgate(sel, SEL_VPE, virt, PAGE_SIZE as goff * 5, Perm::W),
            Code::InvArgs
        );
        wv_assert_err!(
            syscalls::create_mgate(
                sel,
                SEL_VPE,
                virt + PAGE_SIZE as goff,
                PAGE_SIZE as goff * 4,
                Perm::W
            ),
            Code::InvArgs
        );
    }

    // the TCU region is off limits
    #[cfg(target_os = "none")]
    wv_assert_err!(
        syscalls::create_mgate(
            sel,
            SEL_VPE,
            m3::tcu::MMIO_ADDR as goff,
            PAGE_SIZE as goff,
            Perm::R
        ),
        Code::InvArgs
    );
}

fn create_rgate() {
    let sel = VPE::cur().alloc_sel();

    // invalid dest selector
    wv_assert_err!(syscalls::create_rgate(SEL_VPE, 10, 10), Code::InvArgs);
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
    let srv = VPE::cur().alloc_sel();
    let mut rgate = wv_assert_ok!(RecvGate::new(10, 10));
    wv_assert_ok!(rgate.activate());
    wv_assert_ok!(syscalls::create_srv(srv, rgate.sel(), "test", 0,));

    let sel = VPE::cur().alloc_sel();

    // invalid dest selector
    wv_assert_err!(
        syscalls::create_sess(SEL_VPE, srv, 0, 0, false),
        Code::InvArgs
    );
    // invalid service selector
    wv_assert_err!(
        syscalls::create_sess(sel, SEL_VPE, 0, 0, false),
        Code::InvArgs
    );

    wv_assert_ok!(syscalls::revoke(
        VPE::cur().sel(),
        CapRngDesc::new(CapType::OBJECT, srv, 1),
        true
    ));
}

#[allow(clippy::cognitive_complexity)]
fn create_map() {
    if !VPE::cur().pe_desc().has_virtmem() {
        return;
    }

    let meminv = wv_assert_ok!(MemGate::new(64, Perm::RW)); // not page-granular
    let mem = wv_assert_ok!(MemGate::new(PAGE_SIZE * 4, Perm::RW));

    // invalid VPE selector
    wv_assert_err!(
        syscalls::create_map(0, SEL_KMEM, mem.sel(), 0, 4, Perm::RW),
        Code::InvArgs
    );
    // invalid memgate selector
    wv_assert_err!(
        syscalls::create_map(0, VPE::cur().sel(), SEL_VPE, 0, 4, Perm::RW),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::create_map(0, VPE::cur().sel(), meminv.sel(), 0, 4, Perm::RW),
        Code::InvArgs
    );
    // invalid first page
    wv_assert_err!(
        syscalls::create_map(0, VPE::cur().sel(), mem.sel(), 4, 4, Perm::RW),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::create_map(0, VPE::cur().sel(), mem.sel(), !0, 4, Perm::RW),
        Code::InvArgs
    );
    // invalid page count
    wv_assert_err!(
        syscalls::create_map(0, VPE::cur().sel(), mem.sel(), 0, 5, Perm::RW),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::create_map(0, VPE::cur().sel(), mem.sel(), 3, 2, Perm::RW),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::create_map(0, VPE::cur().sel(), mem.sel(), 4, 0, Perm::RW),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::create_map(0, VPE::cur().sel(), mem.sel(), !0, !0, Perm::RW),
        Code::InvArgs
    );
    // invalid permissions
    wv_assert_err!(
        syscalls::create_map(0, VPE::cur().sel(), mem.sel(), 0, 4, Perm::X),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::create_map(0, VPE::cur().sel(), mem.sel(), 0, 4, Perm::RWX),
        Code::InvArgs
    );
}

#[allow(clippy::cognitive_complexity)]
fn create_vpe() {
    let sel = VPE::cur().alloc_sel();
    let rgate = wv_assert_ok!(RecvGate::new(10, 10));
    let sgate = wv_assert_ok!(SendGate::new(&rgate));
    let kmem = VPE::cur().kmem().sel();

    let pe = wv_assert_ok!(PE::new(VPE::cur().pe_desc()));

    // invalid dest selector
    wv_assert_err!(
        syscalls::create_vpe(SEL_KMEM, INVALID_SEL, INVALID_SEL, "test", pe.sel(), kmem),
        Code::InvArgs
    );

    if VPE::cur().pe_desc().has_virtmem() {
        // invalid sgate
        wv_assert_err!(
            syscalls::create_vpe(sel, SEL_VPE, INVALID_SEL, "test", pe.sel(), kmem),
            Code::InvArgs
        );
    }

    // invalid name
    wv_assert_err!(
        syscalls::create_vpe(sel, sgate.sel(), INVALID_SEL, "", pe.sel(), kmem),
        Code::InvArgs
    );

    // invalid kmem
    wv_assert_err!(
        syscalls::create_vpe(sel, sgate.sel(), INVALID_SEL, "test", pe.sel(), INVALID_SEL),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::create_vpe(sel, sgate.sel(), INVALID_SEL, "test", pe.sel(), SEL_VPE),
        Code::InvArgs
    );
}

fn create_sem() {
    let sel = VPE::cur().alloc_sel();

    // invalid selector
    wv_assert_err!(syscalls::create_sem(SEL_VPE, 0), Code::InvArgs);
    wv_assert_ok!(syscalls::create_sem(sel, 1));
    // one down does not block us
    wv_assert_ok!(syscalls::sem_ctrl(sel, SemOp::DOWN));

    wv_assert_ok!(VPE::cur().revoke(CapRngDesc::new(CapType::OBJECT, sel, 1), false));
}

fn alloc_ep() {
    let sel = VPE::cur().alloc_sel();

    // try to use the EP object after the VPE we allocated it for is gone
    {
        {
            let pe = wv_assert_ok!(PE::new(VPE::cur().pe_desc()));
            let vpe = wv_assert_ok!(VPE::new_with(pe, VPEArgs::new("test")));
            wv_assert_ok!(syscalls::alloc_ep(sel, vpe.sel(), TOTAL_EPS, 1));
        }

        let mgate = wv_assert_ok!(MemGate::new(0x1000, Perm::RW));
        wv_assert_err!(
            syscalls::activate(sel, mgate.sel(), INVALID_SEL, 0),
            Code::InvArgs
        );
    }

    // invalid dest selector
    wv_assert_err!(
        syscalls::alloc_ep(SEL_VPE, VPE::cur().pe().sel(), TOTAL_EPS, 1),
        Code::InvArgs
    );
    // invalid VPE selector
    wv_assert_err!(syscalls::alloc_ep(sel, SEL_PE, TOTAL_EPS, 1), Code::InvArgs);
    // invalid reply count
    wv_assert_err!(
        syscalls::alloc_ep(sel, VPE::cur().sel(), AVAIL_EPS - 2, !0),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::alloc_ep(sel, VPE::cur().sel(), AVAIL_EPS - 2, TOTAL_EPS as u32),
        Code::InvArgs
    );

    // any EP
    let ep = wv_assert_ok!(syscalls::alloc_ep(sel, VPE::cur().sel(), TOTAL_EPS, 1));
    wv_assert!(ep >= FIRST_USER_EP);
    wv_assert!(ep < TOTAL_EPS);
    wv_assert_ok!(VPE::cur().revoke(CapRngDesc::new(CapType::OBJECT, sel, 1), false));

    // specific EP
    let ep = wv_assert_ok!(syscalls::alloc_ep(sel, VPE::cur().sel(), AVAIL_EPS - 2, 1));
    wv_assert_eq!(ep, AVAIL_EPS - 2);
    wv_assert_ok!(VPE::cur().revoke(CapRngDesc::new(CapType::OBJECT, sel, 1), false));
}

fn activate() {
    let ep1 = wv_assert_ok!(VPE::cur().epmng_mut().acquire(0));
    let ep2 = wv_assert_ok!(VPE::cur().epmng_mut().acquire(0));
    let sel = VPE::cur().alloc_sel();
    let mgate = wv_assert_ok!(MemGate::new(0x1000, Perm::RW));

    // invalid EP sel
    wv_assert_err!(
        syscalls::activate(SEL_VPE, mgate.sel(), INVALID_SEL, 0),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::activate(sel, mgate.sel(), INVALID_SEL, 0),
        Code::InvArgs
    );
    // invalid mgate sel
    wv_assert_err!(
        syscalls::activate(ep1.sel(), SEL_VPE, INVALID_SEL, 0),
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

    VPE::cur().epmng_mut().release(ep2, true);
    VPE::cur().epmng_mut().release(ep1, true);
}

fn derive_mem() {
    let vpe = VPE::cur().sel();
    let sel = VPE::cur().alloc_sel();
    let mem = wv_assert_ok!(MemGate::new(0x4000, Perm::RW));

    // invalid dest selector
    wv_assert_err!(
        syscalls::derive_mem(vpe, SEL_VPE, mem.sel(), 0, 0x1000, Perm::RW),
        Code::InvArgs
    );
    // invalid mem
    wv_assert_err!(
        syscalls::derive_mem(vpe, sel, SEL_VPE, 0, 0x1000, Perm::RW),
        Code::InvArgs
    );
    // invalid offset
    wv_assert_err!(
        syscalls::derive_mem(vpe, sel, mem.sel(), 0x4000, 0x1000, Perm::RW),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::derive_mem(vpe, sel, mem.sel(), !0, 0x1000, Perm::RW),
        Code::InvArgs
    );
    // invalid size
    wv_assert_err!(
        syscalls::derive_mem(vpe, sel, mem.sel(), 0, 0x4001, Perm::RW),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::derive_mem(vpe, sel, mem.sel(), 0x2000, 0x2001, Perm::RW),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::derive_mem(vpe, sel, mem.sel(), 0x2000, 0, Perm::RW),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::derive_mem(vpe, sel, mem.sel(), 0x4000, 0, Perm::RW),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::derive_mem(vpe, sel, mem.sel(), !0, !0, Perm::RW),
        Code::InvArgs
    );
    // perms are arbitrary; will be ANDed
}

fn derive_kmem() {
    let sel = VPE::cur().alloc_sel();
    let quota = wv_assert_ok!(VPE::cur().kmem().quota());

    // invalid dest selector
    wv_assert_err!(
        syscalls::derive_kmem(VPE::cur().kmem().sel(), SEL_VPE, quota / 2),
        Code::InvArgs
    );
    // invalid quota
    wv_assert_err!(
        syscalls::derive_kmem(VPE::cur().kmem().sel(), sel, quota + 1),
        Code::NoSpace
    );
    // invalid kmem sel
    wv_assert_err!(
        syscalls::derive_kmem(SEL_VPE, sel, quota + 1),
        Code::InvArgs
    );

    // do that test twice, because we might cause pagefaults during the first test, changing the
    // kernel memory quota (our pager shares the kmem with us).
    for i in 0..=1 {
        let before = wv_assert_ok!(VPE::cur().kmem().quota());
        // transfer memory
        {
            let kmem2 = wv_assert_ok!(VPE::cur().kmem().derive(before / 2));
            let quota2 = wv_assert_ok!(kmem2.quota());
            let nquota = wv_assert_ok!(VPE::cur().kmem().quota());
            wv_assert_eq!(quota2, before / 2);
            // we don't know exactly, because we have paid for the new cap and kobject too
            wv_assert!(nquota <= before / 2);
        }
        // only do the check in the second test where no pagefaults should occur
        if i == 1 {
            let nquota = wv_assert_ok!(VPE::cur().kmem().quota());
            wv_assert_eq!(nquota, before);
        }
    }

    let kmem = wv_assert_ok!(VPE::cur().kmem().derive(quota / 2));
    {
        let pe = wv_assert_ok!(PE::new(VPE::cur().pe_desc()));
        let _vpe = wv_assert_ok!(VPE::new_with(pe, VPEArgs::new("test").kmem(kmem.clone())));
        // VPE is still using the kmem
        wv_assert_err!(
            VPE::cur().revoke(CapRngDesc::new(CapType::OBJECT, kmem.sel(), 1), false),
            Code::NotRevocable
        );
    }

    // now we can revoke it
    wv_assert_ok!(VPE::cur().revoke(CapRngDesc::new(CapType::OBJECT, kmem.sel(), 1), false));
}

fn derive_pe() {
    let sel = VPE::cur().alloc_sel();
    let pe = wv_assert_ok!(PE::new(VPE::cur().pe_desc()));
    let oquota = wv_assert_ok!(pe.quota());

    // invalid dest selector
    wv_assert_err!(syscalls::derive_pe(pe.sel(), SEL_VPE, 1), Code::InvArgs);
    // invalid ep count
    wv_assert_err!(
        syscalls::derive_pe(pe.sel(), sel, oquota + 1),
        Code::NoSpace
    );
    // invalid pe sel
    wv_assert_err!(syscalls::derive_pe(SEL_VPE, sel, 1), Code::InvArgs);

    // transfer EPs
    {
        let pe2 = wv_assert_ok!(pe.derive(1));
        let quota2 = wv_assert_ok!(pe2.quota());
        let nquota = wv_assert_ok!(pe.quota());
        wv_assert_eq!(quota2, 1);
        wv_assert_eq!(nquota, oquota - 1);
    }
    let nquota = wv_assert_ok!(pe.quota());
    wv_assert_eq!(nquota, oquota);

    {
        let _vpe = wv_assert_ok!(VPE::new(pe.clone(), "test"));
        // VPE is still using the PE
        wv_assert_err!(
            VPE::cur().revoke(CapRngDesc::new(CapType::OBJECT, pe.sel(), 1), false),
            Code::NotRevocable
        );
    }

    // now we can revoke it
    wv_assert_ok!(VPE::cur().revoke(CapRngDesc::new(CapType::OBJECT, pe.sel(), 1), false));
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
    let crd = CapRngDesc::new(CapType::OBJECT, VPE::cur().alloc_sels(2), 2);
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
    let sel = VPE::cur().alloc_sel();
    let mut hdl = DummyHandler {
        sessions: SessionContainer::new(16),
    };
    let srv = wv_assert_ok!(Server::new_private("test", &mut hdl));

    let _sess1 = wv_assert_ok!(ServerSession::new(srv.sel(), 0, 0xDEAD_BEEF, false));
    let _sess2 = wv_assert_ok!(ServerSession::new(srv.sel(), 1, 0x1234, false));

    // dummy VPE that should receive the session
    let pe = wv_assert_ok!(PE::new(VPE::cur().pe_desc()));
    let vpe = wv_assert_ok!(VPE::new(pe, "test"));

    // invalid service selector
    wv_assert_err!(
        syscalls::get_sess(SEL_KMEM, vpe.sel(), sel, 0xDEAD_BEEF),
        Code::InvArgs
    );
    // invalid VPE selector
    wv_assert_err!(
        syscalls::get_sess(srv.sel(), SEL_KMEM, sel, 0xDEAD_BEEF),
        Code::InvArgs
    );
    // own VPE selector
    wv_assert_err!(
        syscalls::get_sess(srv.sel(), VPE::cur().sel(), sel, 0xDEAD_BEEF),
        Code::InvArgs
    );
    // invalid destination selector
    wv_assert_err!(
        syscalls::get_sess(srv.sel(), vpe.sel(), SEL_KMEM, 0xDEAD_BEEF),
        Code::InvArgs
    );
    // unknown session
    wv_assert_err!(
        syscalls::get_sess(srv.sel(), vpe.sel(), sel, 0x2222),
        Code::InvArgs
    );
    // not our session
    wv_assert_err!(
        syscalls::get_sess(srv.sel(), vpe.sel(), sel, 0x1234),
        Code::NoPerm
    );

    // success
    wv_assert_ok!(syscalls::get_sess(srv.sel(), vpe.sel(), sel, 0xDEAD_BEEF));
}

fn kmem_quota() {
    // invalid selector
    wv_assert_err!(syscalls::kmem_quota(SEL_VPE), Code::InvArgs);
    wv_assert_err!(syscalls::kmem_quota(VPE::cur().alloc_sel()), Code::InvArgs);
}

fn pe_quota() {
    // invalid selector
    wv_assert_err!(syscalls::pe_quota(SEL_VPE), Code::InvArgs);
    wv_assert_err!(syscalls::pe_quota(VPE::cur().alloc_sel()), Code::InvArgs);
}

fn sem_ctrl() {
    // invalid selector
    wv_assert_err!(syscalls::sem_ctrl(SEL_VPE, SemOp::DOWN), Code::InvArgs);
    wv_assert_err!(
        syscalls::sem_ctrl(VPE::cur().alloc_sel(), SemOp::DOWN),
        Code::InvArgs
    );
}

fn vpe_ctrl() {
    wv_assert_err!(syscalls::vpe_ctrl(SEL_KMEM, VPEOp::START, 0), Code::InvArgs);
    wv_assert_err!(
        syscalls::vpe_ctrl(INVALID_SEL, VPEOp::START, 0),
        Code::InvArgs
    );
    // can't start ourself
    wv_assert_err!(
        syscalls::vpe_ctrl(VPE::cur().sel(), VPEOp::START, 0),
        Code::InvArgs
    );
}

fn vpe_wait() {
    wv_assert_err!(syscalls::vpe_wait(&[], 0), Code::InvArgs);
}

fn exchange() {
    let pe = wv_assert_ok!(PE::new(VPE::cur().pe_desc()));
    let mut child = wv_assert_ok!(VPE::new(pe, "test"));
    let csel = child.alloc_sel();

    let sel = VPE::cur().alloc_sel();
    let unused = CapRngDesc::new(CapType::OBJECT, sel, 1);
    let used = CapRngDesc::new(CapType::OBJECT, 0, 1);

    // invalid VPE sel
    wv_assert_err!(
        syscalls::exchange(SEL_KMEM, used, csel, false),
        Code::InvArgs
    );
    // invalid own caps (source caps can be invalid)
    wv_assert_err!(
        syscalls::exchange(VPE::cur().sel(), used, unused.start(), true),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::exchange(child.sel(), used, 0, true),
        Code::InvArgs
    );
    // invalid other caps
    wv_assert_err!(
        syscalls::exchange(VPE::cur().sel(), used, 0, false),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::exchange(child.sel(), used, 0, false),
        Code::InvArgs
    );
}

fn delegate() {
    let m3fs = wv_assert_ok!(M3FS::new("m3fs-clone"));
    let m3fs = m3fs.borrow();
    let sess = m3fs.as_any().downcast_ref::<M3FS>().unwrap().sess();
    let crd = CapRngDesc::new(CapType::OBJECT, SEL_VPE, 1);

    // invalid VPE selector
    wv_assert_err!(
        syscalls::delegate(SEL_KMEM, sess.sel(), crd, |_| {}, |_| Ok(())),
        Code::InvArgs
    );
    // invalid sess selector
    wv_assert_err!(
        syscalls::delegate(VPE::cur().sel(), SEL_VPE, crd, |_| {}, |_| Ok(())),
        Code::InvArgs
    );
    // CRD can be anything (depends on server)
}

fn obtain() {
    let m3fs = wv_assert_ok!(M3FS::new("m3fs-clone"));
    let m3fs = m3fs.borrow();
    let sess = m3fs.as_any().downcast_ref::<M3FS>().unwrap().sess();
    let sel = VPE::cur().alloc_sel();
    let crd = CapRngDesc::new(CapType::OBJECT, sel, 1);
    let inval = CapRngDesc::new(CapType::OBJECT, SEL_VPE, 1);

    // invalid VPE selector
    wv_assert_err!(
        syscalls::obtain(SEL_KMEM, sess.sel(), crd, |_| {}, |_| Ok(())),
        Code::InvArgs
    );
    // invalid sess selector
    wv_assert_err!(
        syscalls::obtain(VPE::cur().sel(), SEL_VPE, crd, |_| {}, |_| Ok(())),
        Code::InvArgs
    );
    // invalid CRD
    wv_assert_err!(
        syscalls::obtain(VPE::cur().sel(), sess.sel(), inval, |_| {}, |_| Ok(())),
        Code::InvArgs
    );
}

fn revoke() {
    let crd_pe = CapRngDesc::new(CapType::OBJECT, SEL_PE, 1);
    let crd_vpe = CapRngDesc::new(CapType::OBJECT, SEL_VPE, 1);
    let crd_mem = CapRngDesc::new(CapType::OBJECT, SEL_KMEM, 1);

    // invalid VPE selector
    wv_assert_err!(syscalls::revoke(SEL_KMEM, crd_vpe, true), Code::InvArgs);
    // can't revoke PE, VPE, or mem cap
    wv_assert_err!(
        syscalls::revoke(VPE::cur().sel(), crd_pe, true),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::revoke(VPE::cur().sel(), crd_vpe, true),
        Code::InvArgs
    );
    wv_assert_err!(
        syscalls::revoke(VPE::cur().sel(), crd_mem, true),
        Code::InvArgs
    );
}
