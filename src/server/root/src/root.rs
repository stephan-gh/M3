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

#![no_std]

#[macro_use]
extern crate m3;
extern crate resmng;
extern crate thread;

mod loader;

use m3::cap::Selector;
use m3::cell::{LazyStaticCell, RefCell, StaticCell};
use m3::cfg::PAGE_SIZE;
use m3::com::{MemGate, RGateArgs, RecvGate, SGateArgs, SendGate};
use m3::errors::{Code, Error};
use m3::goff;
use m3::kif;
use m3::math;
use m3::pes::{VPEArgs, VPE};
use m3::rc::Rc;
use m3::session::ResMng;
use m3::syscalls;
use m3::tcu;

use resmng::childs::{self, Child, OwnChild};
use resmng::{memory, requests, sendqueue, subsys};

static SUBSYS: LazyStaticCell<subsys::Subsystem> = LazyStaticCell::default();
static BMODS: StaticCell<u64> = StaticCell::new(0);

fn find_mod(name: &str) -> Option<(MemGate, usize)> {
    SUBSYS
        .mods()
        .iter()
        .enumerate()
        .position(|(idx, m)| (BMODS.get() & (1 << idx)) == 0 && m.name() == name)
        .map(|idx| {
            BMODS.set(BMODS.get() | 1 << idx);
            (SUBSYS.get_mod(idx), SUBSYS.mods()[idx].size as usize)
        })
}

fn start_child(child: &mut OwnChild) -> Result<(), Error> {
    let bmod = find_mod(child.cfg().name()).ok_or_else(|| Error::new(Code::NotFound))?;

    #[allow(clippy::identity_conversion)]
    let sgate = SendGate::new_with(
        SGateArgs::new(requests::rgate())
            .credits(1)
            .label(tcu::Label::from(child.id())),
    )?;

    let mut vpe = VPE::new_with(
        child.pe().unwrap().pe_obj(),
        VPEArgs::new(child.name())
            .resmng(ResMng::new(sgate))
            .kmem(child.kmem().clone()),
    )?;

    if let Some(fs) = VPE::cur().mounts().get_by_path("/") {
        vpe.mounts().add("/", fs)?;
        vpe.obtain_mounts()?;
    }

    if let Some(sub) = child.subsys() {
        sub.finalize(&mut vpe)
            .expect("Unable to finalize subsystem");
    }

    let mut bmapper = loader::BootMapper::new(
        vpe.sel(),
        bmod.0.sel(),
        vpe.pe_desc().has_virtmem(),
        child.mem().clone(),
    );
    let bfile = loader::BootFile::new(bmod.0, bmod.1);
    let bfileref = VPE::cur().files().add(Rc::new(RefCell::new(bfile)))?;
    child.start(vpe, &mut bmapper, bfileref)?;

    for a in bmapper.fetch_allocs() {
        child.add_mem(a, None);
    }

    Ok(())
}

fn create_rgate(buf_size: usize, msg_size: usize) -> Result<RecvGate, Error> {
    // allocate and map memory for receive buffer. note that we need to do that manually here,
    // because RecvBufs allocate new physical memory via the resource manager and root does not have
    // a resource manager.
    let (rbuf_addr, _) = VPE::cur().pe_desc().rbuf_space();
    let (rbuf_off, rbuf_mem) = if VPE::cur().pe_desc().has_virtmem() {
        let buf_mem = memory::container().alloc_mem(buf_size as goff)?;
        let pages = (buf_mem.capacity() as usize + PAGE_SIZE - 1) / PAGE_SIZE;
        syscalls::create_map(
            (rbuf_addr / PAGE_SIZE) as Selector,
            VPE::cur().sel(),
            buf_mem.sel(),
            0,
            pages as Selector,
            kif::Perm::R,
        )?;
        (0, Some(buf_mem.sel()))
    }
    else {
        (rbuf_addr, None)
    };

    let mut rgate = RecvGate::new_with(
        RGateArgs::default()
            .order(math::next_log2(buf_size))
            .msg_order(math::next_log2(msg_size)),
    )?;
    rgate.activate_with(rbuf_mem, rbuf_off, rbuf_addr)?;
    Ok(rgate)
}

fn workloop() {
    requests::workloop(|| {}, start_child).expect("Running the workloop failed");
}

#[no_mangle]
pub fn main() -> i32 {
    SUBSYS.set(subsys::Subsystem::new().expect("Unable to read subsystem info"));

    let req_rgate = create_rgate(1 << 12, 1 << 8).expect("Unable to create request RecvGate");
    requests::init(req_rgate);

    let squeue_rgate = create_rgate(sendqueue::RBUF_SIZE, sendqueue::RBUF_MSG_SIZE)
        .expect("Unable to create sendqueue RecvGate");
    sendqueue::init(squeue_rgate);

    thread::init();
    // TODO calculate the number of threads we need (one per child?)
    for _ in 0..8 {
        thread::ThreadManager::get().add_thread(workloop as *const () as usize, 0);
    }

    SUBSYS
        .start(start_child)
        .expect("Unable to start subsystem");

    childs::get().start_waiting(1);

    workloop();

    log!(resmng::LOG_DEF, "All childs gone. Exiting.");

    0
}
