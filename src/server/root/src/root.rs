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

mod config;
mod loader;
mod requests;

use m3::boxed::Box;
use m3::cap::Selector;
use m3::cell::{LazyStaticCell, RefCell, StaticCell};
use m3::cfg;
use m3::col::{String, Vec};
use m3::com::{MemGate, RGateArgs, RecvGate, SGateArgs, SendGate};
use m3::errors::Error;
use m3::goff;
use m3::kif::{self, PEDesc, PEType};
use m3::math;
use m3::pes::{VPEArgs, VPE};
use m3::rc::Rc;
use m3::session::ResMng;
use m3::subsys;
use m3::syscalls;
use m3::tcu;

use config::Config;
use resmng::childs::{self, Child, Id, OwnChild};
use resmng::{memory, pes, sendqueue};

struct ChildDesc {
    child: OwnChild,
    mem: MemGate,
    size: usize,
}

static DELAYED: StaticCell<Vec<ChildDesc>> = StaticCell::new(Vec::new());
static RGATE: LazyStaticCell<RecvGate> = LazyStaticCell::default();
static OUR_PE: StaticCell<Option<Rc<pes::PEUsage>>> = StaticCell::new(None);

fn start_child(mut desc: ChildDesc) -> Result<(), Error> {
    #[allow(clippy::identity_conversion)]
    let sgate = SendGate::new_with(
        SGateArgs::new(&RGATE)
            .credits(1)
            .label(tcu::Label::from(desc.child.id())),
    )?;

    let vpe = VPE::new_with(
        desc.child.pe().unwrap().pe_obj(),
        VPEArgs::new(desc.child.name())
            .resmng(ResMng::new(sgate))
            .kmem(desc.child.kmem().clone()),
    )?;

    let mem_pool = desc.child.mem().clone();
    let mut bmapper = loader::BootMapper::new(
        vpe.sel(),
        desc.mem.sel(),
        vpe.pe_desc().has_virtmem(),
        mem_pool,
    );
    let bfile = loader::BootFile::new(desc.mem, desc.size);
    let bfileref = VPE::cur().files().add(Rc::new(RefCell::new(bfile)))?;

    desc.child.start(vpe, &mut bmapper, bfileref)?;

    for a in bmapper.fetch_allocs() {
        desc.child.add_mem(a, None);
    }

    childs::get().add(Box::new(desc.child));
    Ok(())
}

fn start_delayed() {
    let mut new_wait = false;
    let mut idx = 0;
    let delayed = DELAYED.get_mut();
    while idx < delayed.len() {
        if delayed[idx].child.has_unmet_reqs() {
            idx += 1;
            continue;
        }

        start_child(delayed.remove(idx)).expect("Unable to start boot module");
        new_wait = true;
    }

    if new_wait {
        childs::get().start_waiting(1);
    }
}

fn start_boot_mods(subsys: &subsys::Subsystem, mut mems: memory::MemModCon) {
    let mut cfg_mem: Option<(MemGate, goff)> = None;

    // find boot config
    for (id, m) in subsys.mods().iter().enumerate() {
        if m.name() == "boot.xml" {
            cfg_mem = Some((subsys.get_mod(id), m.size));
            continue;
        }
    }

    // read boot config
    let cfg_mem = cfg_mem.unwrap();
    let mut xml: Vec<u8> = Vec::with_capacity(cfg_mem.1 as usize);

    // safety: will be initialized by read below
    unsafe { xml.set_len(cfg_mem.1 as usize) };
    cfg_mem.0.read(&mut xml, 0).expect("Unable to read boot config");

    // parse boot config
    let xml_str = String::from_utf8(xml).expect("Unable to convert boot config to UTF-8 string");
    let cfg = Config::parse(&xml_str, true).expect("Unable to parse boot config");
    log!(resmng::LOG_CFG, "Parsed {:?}", cfg);
    cfg.check();

    let args = cfg.parse_args();

    // keep our own PE to make sure that we allocate a different one for the next domain in case
    // our domain contains just ourself.
    if !args.share_pe {
        OUR_PE.set(Some(Rc::new(
            pes::get()
                .find_and_alloc(VPE::cur().pe_desc())
                .expect("Unable to find own PE"),
        )));
    }
    else {
        if !VPE::cur().pe_desc().has_virtmem() {
            panic!("Can't share root's PE without VM support");
        }
    }

    // determine default mem and kmem per child
    let (def_kmem, def_umem) = cfg.split_mem(&mems);

    let mut id = 0;
    let mut moditer = subsys.mods().iter();
    for d in cfg.root().domains().iter() {
        // we need virtual memory support for multiple apps per domain
        let cur_desc = VPE::cur().pe_desc();
        let pe_desc = if d.apps().len() > 1 {
            PEDesc::new(PEType::COMP_EMEM, cur_desc.isa(), 0)
        }
        else {
            cur_desc
        };
        let pe_usage = Rc::new(
            pes::get()
                .find_and_alloc(pe_desc)
                .expect("Unable to find free and suitable PE"),
        );

        let def_eps = Config::split_eps(&pe_usage.pe_obj(), &d).expect("Unable to split EPs");

        for cfg in d.apps() {
            let m = loop {
                let m = moditer.next().unwrap();
                if m.name() != "boot.xml" && !m.name().starts_with("root") {
                    break m;
                }
                id += 1;
            };

            // determine PE object with potentially reduced number of EPs
            let pe_usage = if cfg.eps().is_none() {
                pe_usage.clone()
            }
            else {
                Rc::new(
                    pe_usage
                        .derive(cfg.eps().unwrap_or(def_eps))
                        .expect("Unable to derive new PE"),
                )
            };

            // kernel memory for child
            let kmem = if cfg.kernel_mem().is_none() && args.share_kmem {
                VPE::cur().kmem().clone()
            }
            else {
                let kmem_bytes = cfg.kernel_mem().unwrap_or(def_kmem);
                VPE::cur()
                    .kmem()
                    .derive(kmem_bytes)
                    .expect("Unable to derive new kernel memory")
            };

            // memory pool for child
            let user_mem = cfg.user_mem().unwrap_or(def_umem as usize) as goff;
            let mem_pool = Rc::new(RefCell::new(
                mems.alloc_pool(user_mem)
                    .expect("Unable to allocate memory pool"),
            ));
            // add requested physical memory regions to pool
            for mem in cfg.phys_mems() {
                let mslice = mems.find_mem(mem.phys(), mem.size()).unwrap_or_else(|_| {
                    panic!("Unable to find memory {:#x}:{:#x}", mem.phys(), mem.size())
                });
                mem_pool.borrow_mut().add(mslice);
            }

            let child = OwnChild::new(
                id as Id,
                pe_usage,
                // TODO either remove args and daemon from config or remove the clones from OwnChild
                cfg.args().clone(),
                cfg.daemon(),
                kmem,
                mem_pool,
                cfg.clone(),
            );
            log!(resmng::LOG_CHILD, "Created {:?}", child);

            let desc = ChildDesc {
                child,
                mem: subsys.get_mod(id),
                size: m.size as usize,
            };
            if desc.child.has_unmet_reqs() {
                DELAYED.get_mut().push(desc);
            }
            else {
                start_child(desc).expect("Unable to start boot module");
            }
            id += 1;
        }
    }
}

#[no_mangle]
pub fn main() -> i32 {
    let subsys = subsys::Subsystem::new().expect("Unable to read subsystem info");

    log!(resmng::LOG_DEF, "Boot modules:");
    for m in subsys.mods() {
        log!(resmng::LOG_DEF, "  {:?}", m);
    }

    log!(resmng::LOG_DEF, "Available PEs:");
    for (i, pe) in subsys.pes().iter().enumerate() {
        log!(resmng::LOG_DEF, "  {:?}", pe);
        pes::get().add(pe.id as tcu::PEId, subsys.get_pe(i));
    }

    log!(resmng::LOG_DEF, "Available memory:");
    let mut memcon = memory::MemModCon::default();
    for (i, mem) in subsys.mems().iter().enumerate() {
        let mem_mod = Rc::new(memory::MemMod::new(
            subsys.get_mem(i),
            mem.addr(),
            mem.size(),
            mem.reserved(),
        ));
        log!(resmng::LOG_DEF, "  {:?}", mem_mod);
        memcon.add(mem_mod);
    }

    // allocate and map memory for receive buffers. note that we need to do that manually here,
    // because RecvBufs allocate new physical memory via the resource manager and root does not have
    // a resource manager.
    let rgate_size = 1 << 12;
    let buf_mem = memcon
        .alloc_mem((rgate_size + sendqueue::RBUF_SIZE) as u64)
        .expect("Unable to allocate mem for receive buffers");
    let (mut rbuf_addr, _) = VPE::cur().pe_desc().rbuf_space();
    let (mut rbuf_off, rbuf_mem) = if VPE::cur().pe_desc().has_virtmem() {
        let pages = (buf_mem.capacity() as usize + cfg::PAGE_SIZE - 1) / cfg::PAGE_SIZE;
        syscalls::create_map(
            (rbuf_addr / cfg::PAGE_SIZE) as Selector,
            VPE::cur().sel(),
            buf_mem.sel(),
            0,
            pages as Selector,
            kif::Perm::R,
        )
        .expect("Unable to map receive buffer");
        (0, Some(buf_mem.sel()))
    }
    else {
        (rbuf_addr, None)
    };

    let mut rgate = RecvGate::new_with(
        RGateArgs::default()
            .order(math::next_log2(rgate_size))
            .msg_order(8),
    )
    .expect("Unable to create RecvGate");
    rgate
        .activate_with(rbuf_mem, rbuf_off, rbuf_addr)
        .expect("Unable to activate RecvGate");
    RGATE.set(rgate);

    rbuf_addr += rgate_size;
    rbuf_off += rgate_size;
    sendqueue::init(Some((rbuf_mem, rbuf_off, rbuf_addr)));

    thread::init();
    // TODO calculate the number of threads we need (one per child?)
    for _ in 0..8 {
        thread::ThreadManager::get().add_thread(requests::workloop as *const () as usize, 0);
    }

    start_boot_mods(&subsys, memcon);

    // ensure that there is no id overlap
    childs::get().set_next_id(subsys.info().mod_count as Id + 1);

    childs::get().start_waiting(1);

    requests::workloop();

    log!(resmng::LOG_DEF, "All childs gone. Exiting.");

    0
}
