/*
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

#![no_std]

mod loader;

use m3::boxed::Box;
use m3::cap::Selector;
use m3::cell::{LazyReadOnlyCell, StaticCell};
use m3::cfg;
use m3::col::ToString;
use m3::com::{MemGate, RGateArgs, RecvGate, SGateArgs, SendGate};
use m3::errors::{Code, Error, VerboseError};
use m3::format;
use m3::goff;
use m3::kif;
use m3::log;
use m3::mem::GlobAddr;
use m3::session::ResMng;
use m3::syscalls;
use m3::tcu;
use m3::tiles::{Activity, ActivityArgs, ChildActivity};
use m3::util::math;
use m3::vfs::FileRef;

use resmng::childs::{self, Child, ChildManager, OwnChild};
use resmng::{config, memory, requests, res::Resources, sendqueue, subsys, tiles};

static SUBSYS: LazyReadOnlyCell<subsys::Subsystem> = LazyReadOnlyCell::default();
static LOADED_BMODS: StaticCell<u64> = StaticCell::new(0);
static PMP_BMODS: StaticCell<u64> = StaticCell::new(0);

fn fetch_mod(mask: &StaticCell<u64>, name: &str) -> Option<(MemGate, GlobAddr, goff)> {
    SUBSYS
        .get()
        .mods()
        .iter()
        .enumerate()
        .position(|(idx, m)| (mask.get() & (1 << idx)) == 0 && m.name() == name)
        .map(|idx| {
            mask.set(mask.get() | 1 << idx);
            let bmod = SUBSYS.get().mods()[idx];
            (
                subsys::Subsystem::get_mod(idx),
                GlobAddr::new(bmod.addr),
                bmod.size,
            )
        })
}

fn modules_range(domain: &config::Domain) -> Result<(GlobAddr, goff), VerboseError> {
    let mut start = goff::MAX;
    let mut end = 0;
    for app in domain.apps() {
        let (_mgate, addr, size) = fetch_mod(&PMP_BMODS, app.name()).ok_or_else(|| {
            VerboseError::new(
                Code::NotFound,
                format!("Unable to find boot module {}", app.name()),
            )
        })?;
        start = start.min(addr.raw());
        end = end.max(addr.raw() + size);
    }
    Ok((GlobAddr::new(start), end - start))
}

struct RootChildStarter {}

impl resmng::subsys::ChildStarter for RootChildStarter {
    fn start(&mut self, res: &mut Resources, child: &mut OwnChild) -> Result<(), VerboseError> {
        let bmod = fetch_mod(&LOADED_BMODS, child.cfg().name())
            .ok_or_else(|| Error::new(Code::NotFound))?;

        #[allow(clippy::useless_conversion)]
        let sgate = SendGate::new_with(
            SGateArgs::new(&requests::rgate())
                .credits(1)
                .label(tcu::Label::from(child.id())),
        )?;

        let mut act = ChildActivity::new_with(
            child.child_tile().unwrap().tile_obj().clone(),
            ActivityArgs::new(child.name())
                .resmng(ResMng::new(sgate))
                .kmem(child.kmem().unwrap()),
        )
        .map_err(|e| VerboseError::new(e.code(), "Unable to create Activity".to_string()))?;

        if Activity::own().mounts().get_by_path("/").is_some() {
            act.add_mount("/", "/");
        }

        let id = child.id();
        if let Some(sub) = child.subsys() {
            sub.finalize_async(res, id, &mut act)
                .expect("Unable to finalize subsystem");
        }

        let mut bmapper = loader::BootMapper::new(
            act.sel(),
            bmod.0.sel(),
            act.tile_desc().has_virtmem(),
            child.mem().pool().clone(),
        );
        let bfile = loader::BootFile::new(bmod.0, bmod.2 as usize);
        let fd = Activity::own().files().add(Box::new(bfile))?;
        child
            .start(act, &mut bmapper, FileRef::new_owned(fd))
            .map_err(|e| VerboseError::new(e.code(), "Unable to start Activity".to_string()))?;

        for a in bmapper.fetch_allocs() {
            child.add_mem(a, None);
        }

        Ok(())
    }

    fn configure_tile(
        &mut self,
        res: &mut Resources,
        tile: &tiles::TileUsage,
        domain: &config::Domain,
    ) -> Result<(), VerboseError> {
        if tile.tile_id() != Activity::own().tile_id() {
            // determine minimum range of boot modules we need to give access to to cover all boot
            // modules that are run on this tile. note that these should always be contiguous,
            // because we collect the boot modules from the config.
            let range = modules_range(domain)?;
            let mslice = res.memory().find_mem(range.0, range.1, kif::Perm::RW)?;

            // create memory gate for this range
            let mgate = mslice.derive().map_err(|e| {
                VerboseError::new(e.code(), "Unable to derive from boot module".to_string())
            })?;

            // configure PMP EP
            tile.add_mem_region(mgate, range.1 as usize, true)
                .map_err(|e| {
                    VerboseError::new(
                        e.code(),
                        "Unable to add PMP region for boot module".to_string(),
                    )
                })
        }
        else {
            // for our own tile there is nothing to do, because we already have a PMP EP that covers
            // all boot modules
            Ok(())
        }
    }
}

fn create_rgate(
    buf_size: usize,
    msg_size: usize,
    rbuf_mem: Option<Selector>,
    rbuf_off: goff,
    rbuf_addr: usize,
) -> Result<RecvGate, Error> {
    let rgate = RecvGate::new_with(
        RGateArgs::default()
            .order(math::next_log2(buf_size))
            .msg_order(math::next_log2(msg_size)),
    )?;
    rgate.activate_with(rbuf_mem, rbuf_off, rbuf_addr)?;
    Ok(rgate)
}

struct WorkloopArgs<'c, 'r> {
    childs: &'c mut ChildManager,
    res: &'r mut Resources,
}

fn workloop(args: &mut WorkloopArgs<'_, '_>) {
    let WorkloopArgs { childs, res } = args;

    requests::workloop(childs, res, |_, _| {}, &mut RootChildStarter {})
        .expect("Running the workloop failed");
}

#[no_mangle]
pub fn main() -> Result<(), Error> {
    let (sub, mut res) = subsys::Subsystem::new().expect("Unable to read subsystem info");
    let args = sub.parse_args();
    for sem in &args.sems {
        res.semaphores()
            .add_sem(sem.clone())
            .expect("Unable to add semaphore");
    }
    SUBSYS.set(sub);

    let max_msg_size = 1 << 8;
    let buf_size = max_msg_size * args.max_clients;

    // allocate and map memory for receive buffer. note that we need to do that manually here,
    // because RecvBufs allocate new physical memory via the resource manager and root does not have
    // a resource manager.
    let (rbuf_addr, _) = Activity::own().tile_desc().rbuf_space();
    let (rbuf_off, rbuf_mem) = if Activity::own().tile_desc().has_virtmem() {
        let buf_mem = res
            .memory()
            .alloc_mem((buf_size + sendqueue::RBUF_SIZE) as goff)
            .expect("Unable to allocate memory for receive buffers");
        let pages = (buf_mem.capacity() as usize + cfg::PAGE_SIZE - 1) / cfg::PAGE_SIZE;
        syscalls::create_map(
            (rbuf_addr / cfg::PAGE_SIZE) as Selector,
            Activity::own().sel(),
            buf_mem.sel(),
            0,
            pages as Selector,
            kif::Perm::R,
        )
        .expect("Unable to map receive buffers");
        (0, Some(buf_mem.sel()))
    }
    else {
        (rbuf_addr as goff, None)
    };

    let req_rgate = create_rgate(buf_size, max_msg_size, rbuf_mem, rbuf_off, rbuf_addr)
        .expect("Unable to create request RecvGate");
    requests::init(req_rgate);

    let squeue_rgate = create_rgate(
        sendqueue::RBUF_SIZE,
        sendqueue::RBUF_MSG_SIZE,
        rbuf_mem,
        rbuf_off + buf_size as goff,
        rbuf_addr + buf_size,
    )
    .expect("Unable to create sendqueue RecvGate");
    sendqueue::init(squeue_rgate);

    let mut childs = childs::ChildManager::default();
    let mut wargs = WorkloopArgs {
        childs: &mut childs,
        res: &mut res,
    };

    thread::init();
    for _ in 0..args.max_clients {
        thread::add_thread(
            workloop as *const () as usize,
            &mut wargs as *mut _ as usize,
        );
    }

    SUBSYS
        .get()
        .start(wargs.childs, wargs.res, &mut RootChildStarter {})
        .expect("Unable to start subsystem");

    wargs.childs.start_waiting(1);

    workloop(&mut wargs);

    log!(resmng::LOG_DEF, "All childs gone. Exiting.");

    Ok(())
}
