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

use m3::boxed::Box;
use m3::cap::Selector;
use m3::cell::{RefCell, StaticCell};
use m3::cfg;
use m3::col::{String, ToString, Vec};
use m3::com::{GateIStream, MemGate, RGateArgs, RecvGate, SGateArgs, SendGate};
use m3::errors::{Code, Error};
use m3::goff;
use m3::kif::{self, boot, PEDesc, PEType};
use m3::math;
use m3::pes::{VPEArgs, PE, VPE};
use m3::rc::Rc;
use m3::session::{ResMng, ResMngOperation};
use m3::tcu;
use m3::util;

use resmng::childs::{self, Child, Id, OwnChild};
use resmng::{config, memory, pes, sems, sendqueue, services};

//
// The kernel initializes our cap space as follows:
// +-----------+-------+-----+-----------+------+-----+----------+-------+-----+-----------+
// | boot info | mod_0 | ... | mod_{n-1} | pe_0 | ... | pe_{n-1} | mem_0 | ... | mem_{n-1} |
// +-----------+-------+-----+-----------+------+-----+----------+-------+-----+-----------+
// ^-- FIRST_FREE_SEL
//
const BOOT_MOD_SELS: Selector = kif::FIRST_FREE_SEL;

static DELAYED: StaticCell<Vec<OwnChild>> = StaticCell::new(Vec::new());
static MODS: StaticCell<(usize, usize)> = StaticCell::new((0, 0));
static RGATE: StaticCell<Option<RecvGate>> = StaticCell::new(None);
static OUR_PE: StaticCell<Option<Rc<pes::PEUsage>>> = StaticCell::new(None);

fn req_rgate() -> &'static RecvGate {
    RGATE.get().as_ref().unwrap()
}

fn reply_result(is: &mut GateIStream, res: Result<(), Error>) {
    match res {
        Err(e) => {
            log!(resmng::LOG_DEF, "request failed: {}", e);
            reply_vmsg!(is, e.code() as u64)
        },
        Ok(_) => reply_vmsg!(is, 0 as u64),
    }
    .expect("Unable to reply");
}

fn reg_serv(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let child_sel: Selector = is.pop()?;
    let dst_sel: Selector = is.pop()?;
    let rgate_sel: Selector = is.pop()?;
    let name: String = is.pop()?;

    let res = services::get().reg_serv(child, child_sel, dst_sel, rgate_sel, name);
    if res.is_ok() && !DELAYED.get().is_empty() {
        start_delayed();
    }
    res
}

fn unreg_serv(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let sel: Selector = is.pop()?;
    let notify: bool = is.pop()?;

    services::get().unreg_serv(child, sel, notify)
}

fn open_session(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let dst_sel: Selector = is.pop()?;
    let name: String = is.pop()?;

    services::get().open_session(child, dst_sel, &name)
}

fn close_session(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let sel: Selector = is.pop()?;

    services::get().close_session(child, sel)
}

fn add_child(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let vpe_sel: Selector = is.pop()?;
    let sgate_sel: Selector = is.pop()?;
    let name: String = is.pop()?;

    child.add_child(vpe_sel, req_rgate(), sgate_sel, name)
}

fn rem_child(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let vpe_sel: Selector = is.pop()?;

    child.rem_child(vpe_sel).map(|_| ())
}

fn alloc_mem(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let dst_sel: Selector = is.pop()?;
    let addr: goff = is.pop()?;
    let size: goff = is.pop()?;
    let perms = kif::Perm::from_bits_truncate(is.pop::<u32>()?);

    if addr == !0 {
        child.alloc_mem(dst_sel, size, perms)
    }
    else {
        child.alloc_mem_at(dst_sel, addr, size, perms)
    }
}

fn free_mem(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let sel: Selector = is.pop()?;

    child.free_mem(sel)
}

fn alloc_pe(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let dst_sel: Selector = is.pop()?;
    let desc = kif::PEDesc::new_from(is.pop()?);

    child
        .alloc_pe(dst_sel, desc)
        .and_then(|desc| reply_vmsg!(is, 0 as u64, desc.value()))
}

fn free_pe(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let sel: Selector = is.pop()?;

    child.free_pe(sel)
}

fn use_sem(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let sel: Selector = is.pop()?;
    let name: String = is.pop()?;

    child.use_sem(&name, sel)
}

fn handle_request(mut is: GateIStream) {
    let op: Result<ResMngOperation, Error> = is.pop();
    let child = childs::get().child_by_id_mut(is.label() as Id).unwrap();

    let res = match op {
        Ok(ResMngOperation::REG_SERV) => reg_serv(&mut is, child),
        Ok(ResMngOperation::UNREG_SERV) => unreg_serv(&mut is, child),

        Ok(ResMngOperation::OPEN_SESS) => open_session(&mut is, child),
        Ok(ResMngOperation::CLOSE_SESS) => close_session(&mut is, child),

        Ok(ResMngOperation::ADD_CHILD) => add_child(&mut is, child),
        Ok(ResMngOperation::REM_CHILD) => rem_child(&mut is, child),

        Ok(ResMngOperation::ALLOC_MEM) => alloc_mem(&mut is, child),
        Ok(ResMngOperation::FREE_MEM) => free_mem(&mut is, child),

        Ok(ResMngOperation::ALLOC_PE) => {
            let res = alloc_pe(&mut is, child);
            if res.is_ok() {
                return;
            }
            res
        },
        Ok(ResMngOperation::FREE_PE) => free_pe(&mut is, child),

        Ok(ResMngOperation::USE_SEM) => use_sem(&mut is, child),

        _ => Err(Error::new(Code::InvArgs)),
    };

    reply_result(&mut is, res);
}

fn start_child(child: &mut OwnChild, bsel: Selector, m: &'static boot::Mod) -> Result<(), Error> {
    let sgate = SendGate::new_with(
        SGateArgs::new(req_rgate())
            .credits(1)
            .label(tcu::Label::from(child.id())),
    )?;

    let vpe = VPE::new_with(
        child.pe().unwrap().pe_obj(),
        VPEArgs::new(child.name())
            .resmng(ResMng::new(sgate))
            .kmem(child.kmem().clone()),
    )?;

    let bfile = loader::BootFile::new(bsel, m.size as usize);
    let mem_pool = child.mem().clone();
    let mut bmapper =
        loader::BootMapper::new(vpe.sel(), bsel, vpe.pe_desc().has_virtmem(), mem_pool);
    let bfileref = VPE::cur().files().add(Rc::new(RefCell::new(bfile)))?;

    child.start(vpe, &mut bmapper, bfileref)?;

    for a in bmapper.fetch_allocs() {
        child.add_mem(a, None);
    }

    Ok(())
}

fn start_delayed() {
    let mut new_wait = false;
    let mut idx = 0;
    let delayed = DELAYED.get_mut();
    while idx < delayed.len() {
        if delayed[idx].has_unmet_reqs() {
            idx += 1;
            continue;
        }

        let mut c = delayed.remove(idx);
        let mods = MODS.get();
        let mut moditer = boot::ModIterator::new(mods.0, mods.1);
        let m = moditer.nth(c.id() as usize).unwrap();
        let sel = BOOT_MOD_SELS + 1 + c.id();
        start_child(&mut c, sel, &m).expect("Unable to start boot module");
        childs::get().add(Box::new(c));
        new_wait = true;
    }

    if new_wait {
        childs::get().start_waiting(1);
    }
}

fn workloop() {
    let thmng = thread::ThreadManager::get();
    let rgate = req_rgate();
    let upcall_rg = RecvGate::upcall();

    loop {
        tcu::TCUIf::sleep().ok();

        let is = rgate.fetch();
        if let Some(is) = is {
            handle_request(is);
        }

        let msg = tcu::TCUIf::fetch_msg(upcall_rg);
        if let Some(msg) = msg {
            childs::get().handle_upcall(msg);
        }

        sendqueue::check_replies();

        if thmng.ready_count() > 0 {
            thmng.try_yield();
        }

        if childs::get().is_empty() {
            break;
        }
    }

    if !thmng.cur().is_main() {
        thmng.stop();
        // just in case there is no ready thread
        m3::exit(0);
    }
}

fn start_boot_mods(mut mems: memory::MemModCon) {
    let mut same_kmem = false;
    let mut cfg_mem: Option<(Id, goff)> = None;

    // find boot config
    let moditer = boot::ModIterator::new(MODS.get().0, MODS.get().1);
    for (id, m) in moditer.enumerate() {
        if m.name() == "boot.xml" {
            cfg_mem = Some((BOOT_MOD_SELS + 1 + id as Id, m.size));
            continue;
        }
    }

    // read boot config
    let cfg_mem = cfg_mem.unwrap();
    let mgate = MemGate::new_bind(cfg_mem.0 as Id);
    let mut xml: Vec<u8> = Vec::with_capacity(cfg_mem.1 as usize);

    // safety: will be initialized by read below
    unsafe { xml.set_len(cfg_mem.1 as usize) };
    mgate.read(&mut xml, 0).expect("Unable to read boot config");

    // parse boot config
    let xml_str = String::from_utf8(xml).expect("Unable to convert boot config to UTF-8 string");
    let cfg = config::Config::parse(&xml_str, true).expect("Unable to parse boot config");
    log!(resmng::LOG_CFG, "Parsed {:?}", cfg);
    cfg.check();

    // determine default mem and kmem per child
    let mut total_mem = mems.capacity();
    let mut total_kmem = VPE::cur()
        .kmem()
        .quota()
        .expect("Unable to determine own quota");
    let mut total_kparties = cfg.count_apps() + 1;
    let mut total_mparties = total_kparties;
    for d in cfg.domains() {
        for a in d.apps() {
            if let Some(kmem) = a.kmem() {
                total_kmem -= kmem;
                total_kparties -= 1;
            }

            let app_mem = a.sum_mem();
            if app_mem != 0 {
                total_mem -= app_mem as goff;
                total_mparties -= 1;
            }

            if a.name().starts_with("root") {
                // parse our own arguments
                for arg in a.args() {
                    if arg == "samekmem" {
                        same_kmem = true;
                    }
                    else if arg.starts_with("sem=") {
                        sems::get()
                            .add_sem(arg[4..].to_string())
                            .expect("Unable to add semaphore");
                    }
                }
            }
        }
    }
    let def_kmem = total_kmem / total_kparties;
    let def_mem = math::round_dn(total_mem / total_mparties as goff, cfg::PAGE_SIZE as goff);

    let mut id = 0;
    let mut moditer = boot::ModIterator::new(MODS.get().0, MODS.get().1);
    for (dom, d) in cfg.domains().iter().enumerate() {
        // we need virtual memory support for multiple apps per domain
        let cur_desc = VPE::cur().pe_desc();
        let pe_desc = if d.apps().len() > 1 {
            if dom == 0 && !cur_desc.has_virtmem() {
                panic!("Can't share root's PE without VM support");
            }
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

        // keep our own PE to make sure that we allocate a different one for the next domain in case
        // our domain contains just ourself.
        if pe_usage.pe_id() == VPE::cur().pe_id() {
            OUR_PE.set(Some(pe_usage.clone()));
        }

        for cfg in d.apps() {
            if cfg.name().starts_with("root") {
                continue;
            }

            let m = loop {
                let m = moditer.next().unwrap();
                if m.name() != "pemux" && m.name() != "boot.xml" && !m.name().starts_with("root") {
                    break m;
                }
                id += 1;
            };

            // kernel memory for child
            let kmem = if cfg.kmem().is_none() && same_kmem {
                VPE::cur().kmem().clone()
            }
            else {
                let kmem_bytes = cfg.kmem().unwrap_or(def_kmem);
                VPE::cur()
                    .kmem()
                    .derive(kmem_bytes)
                    .expect("Unable to derive new kernel memory")
            };

            // memory pool for child
            let child_mem = cfg.sum_mem();
            let child_mem = if child_mem == 0 {
                def_mem
            }
            else {
                child_mem as goff
            };
            let mem_pool = Rc::new(RefCell::new(
                mems.alloc_pool(child_mem)
                    .expect("Unable to allocate memory pool"),
            ));
            // add requested physical memory regions to pool
            for mem in cfg.mems() {
                if let Some(p) = mem.phys() {
                    let mslice = mems.find_mem(p, mem.size()).expect(&format!(
                        "Unable to find memory {:#x}:{:#x}",
                        p,
                        mem.size()
                    ));
                    mem_pool.borrow_mut().add(mslice);
                }
            }

            let mut child = OwnChild::new(
                id as Id,
                pe_usage.clone(),
                // TODO either remove args and daemon from config or remove the clones from OwnChild
                cfg.args().clone(),
                cfg.daemon(),
                kmem,
                mem_pool,
                cfg.clone(),
            );
            log!(resmng::LOG_CHILD, "Created {:?}", child);

            if child.has_unmet_reqs() {
                DELAYED.get_mut().push(child);
            }
            else {
                start_child(&mut child, BOOT_MOD_SELS + 1 + id as Id, &m)
                    .expect("Unable to start boot module");
                childs::get().add(Box::new(child));
            }
            id += 1;
        }
    }
}

#[no_mangle]
pub fn main() -> i32 {
    let mgate = MemGate::new_bind(BOOT_MOD_SELS);
    let mut off: goff = 0;

    let info: boot::Info = mgate.read_obj(0).expect("Unable to read boot info");
    off += util::size_of::<boot::Info>() as goff;

    let mut mods_list = vec![0u8; info.mod_size as usize];
    mgate
        .read(&mut mods_list, off)
        .expect("Unable to read mods");
    off += info.mod_size;

    log!(resmng::LOG_DEF, "Boot modules:");
    MODS.set((
        mods_list.as_slice().as_ptr() as usize,
        info.mod_size as usize,
    ));
    let moditer = boot::ModIterator::new(MODS.get().0, MODS.get().1);
    for m in moditer {
        log!(resmng::LOG_DEF, "  {:?}", m);
    }

    let mut pes: Vec<PEDesc> = Vec::with_capacity(info.pe_count as usize);
    // safety: will be initialized by read below
    unsafe { pes.set_len(info.pe_count as usize) };
    mgate.read(&mut pes, off).expect("Unable to read PEs");

    let pe_sel = BOOT_MOD_SELS + 1 + info.mod_count as Selector;
    let mut user_pes = 0;
    let mut i = 0;
    log!(resmng::LOG_DEF, "Available PEs:");
    for pe in pes {
        log!(
            resmng::LOG_DEF,
            "  PE{:02}: {} {} {} KiB memory",
            i,
            pe.pe_type(),
            pe.isa(),
            pe.mem_size() / 1024
        );
        // skip kernel
        if i >= VPE::cur().pe_id() {
            pes::get().add(i as tcu::PEId, Rc::new(PE::new_bind(pe, pe_sel + i as Selector - 1)));
        }
        if i > 0 && pe.pe_type() != kif::PEType::MEM {
            user_pes += 1;
        }
        i += 1;
    }

    let mut memcon = memory::MemModCon::default();
    let mut mem_sel = BOOT_MOD_SELS + 1 + (user_pes + info.mod_count) as Selector;
    for i in 0..info.mems.len() {
        let mem = &info.mems[i];
        if mem.size() == 0 {
            continue;
        }

        let mem_mod = Rc::new(memory::MemMod::new(
            mem_sel,
            mem.addr(),
            mem.size(),
            mem.reserved(),
        ));
        log!(resmng::LOG_DEF, "Found {:?}", mem_mod);
        memcon.add(mem_mod);
        mem_sel += 1;
    }

    let mut rgate = RecvGate::new_with(RGateArgs::default().order(12).msg_order(8))
        .expect("Unable to create RecvGate");
    rgate.activate().expect("Unable to activate RecvGate");
    RGATE.set(Some(rgate));

    sendqueue::init();
    thread::init();
    // TODO calculate the number of threads we need (one per child?)
    for _ in 0..8 {
        thread::ThreadManager::get().add_thread(workloop as *const () as usize, 0);
    }

    start_boot_mods(memcon);

    // ensure that there is no id overlap
    childs::get().set_next_id(info.mod_count as Id + 1);

    childs::get().start_waiting(1);

    workloop();

    log!(resmng::LOG_DEF, "All childs gone. Exiting.");

    0
}
