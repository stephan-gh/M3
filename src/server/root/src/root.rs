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
use m3::col::{String, ToString, Vec};
use m3::com::{GateIStream, MemGate, RGateArgs, RecvGate, SGateArgs, SendGate};
use m3::dtu;
use m3::errors::Error;
use m3::goff;
use m3::kif::{self, boot, PEDesc};
use m3::pes::{VPEArgs, PE, VPE};
use m3::rc::Rc;
use m3::session::{ResMng, ResMngOperation};
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

fn req_rgate() -> &'static RecvGate {
    RGATE.get().as_ref().unwrap()
}

fn reply_result(is: &mut GateIStream, res: Result<(), Error>) {
    match res {
        Err(e) => {
            log!(RESMNG, "request failed: {}", e);
            reply_vmsg!(is, e.code() as u64)
        },
        Ok(_) => reply_vmsg!(is, 0 as u64),
    }
    .expect("Unable to reply");
}

fn reg_serv(is: &mut GateIStream, child: &mut dyn Child) {
    let child_sel: Selector = is.pop();
    let dst_sel: Selector = is.pop();
    let rgate_sel: Selector = is.pop();
    let name: String = is.pop();

    let res = services::get().reg_serv(child, child_sel, dst_sel, rgate_sel, name);
    if res.is_ok() && !DELAYED.get().is_empty() {
        start_delayed();
    }
    reply_result(is, res);
}

fn unreg_serv(is: &mut GateIStream, child: &mut dyn Child) {
    let sel: Selector = is.pop();
    let notify: bool = is.pop();

    let res = services::get().unreg_serv(child, sel, notify);
    reply_result(is, res);
}

fn open_session(is: &mut GateIStream, child: &mut dyn Child) {
    let dst_sel: Selector = is.pop();
    let name: String = is.pop();

    let res = services::get().open_session(child, dst_sel, &name);
    reply_result(is, res);
}

fn close_session(is: &mut GateIStream, child: &mut dyn Child) {
    let sel: Selector = is.pop();

    let res = services::get().close_session(child, sel);
    reply_result(is, res);
}

fn add_child(is: &mut GateIStream, child: &mut dyn Child) {
    let vpe_sel: Selector = is.pop();
    let sgate_sel: Selector = is.pop();
    let name: String = is.pop();

    let res = child.add_child(vpe_sel, req_rgate(), sgate_sel, name);
    reply_result(is, res);
}

fn rem_child(is: &mut GateIStream, child: &mut dyn Child) {
    let vpe_sel: Selector = is.pop();

    let res = child.rem_child(vpe_sel).map(|_| ());
    reply_result(is, res);
}

fn alloc_mem(is: &mut GateIStream, child: &mut dyn Child) {
    let dst_sel: Selector = is.pop();
    let addr: goff = is.pop();
    let size: usize = is.pop();
    let perms = kif::Perm::from_bits_truncate(is.pop::<u8>());

    let res = if addr == !0 {
        memory::get().allocate_for(child, dst_sel, size, perms)
    }
    else {
        memory::get().allocate_at(child, dst_sel, addr, size)
    };

    reply_result(is, res);
}

fn free_mem(is: &mut GateIStream, child: &mut dyn Child) {
    let sel: Selector = is.pop();

    let res = child.remove_mem(sel);
    reply_result(is, res);
}

fn alloc_pe(is: &mut GateIStream, child: &mut dyn Child) {
    let dst_sel: Selector = is.pop();
    let desc = kif::PEDesc::new_from(is.pop());

    let res = child.alloc_pe(dst_sel, desc);
    match res {
        Err(e) => {
            log!(RESMNG, "request failed: {}", e);
            reply_vmsg!(is, e.code() as u64)
        },
        Ok(desc) => reply_vmsg!(is, 0 as u64, desc.value()),
    }
    .expect("Unable to reply");
}

fn free_pe(is: &mut GateIStream, child: &mut dyn Child) {
    let sel: Selector = is.pop();

    let res = child.free_pe(sel);
    reply_result(is, res);
}

fn start_child(child: &mut OwnChild, bsel: Selector, m: &'static boot::Mod) -> Result<(), Error> {
    let sgate = SendGate::new_with(
        SGateArgs::new(req_rgate())
            .credits(256)
            .label(dtu::Label::from(child.id())),
    )?;

    let pe = pes::get().get(child.pe_id().unwrap());
    let vpe = VPE::new_with(
        pe,
        VPEArgs::new(child.name())
            .resmng(ResMng::new(sgate))
            .kmem(child.kmem().clone()),
    )?;

    let bfile = loader::BootFile::new(bsel, m.size as usize);
    let mut bmapper = loader::BootMapper::new(vpe.sel(), bsel, vpe.pe_desc().has_virtmem());
    let bfileref = VPE::cur().files().add(Rc::new(RefCell::new(bfile)))?;

    child.start(vpe, &mut bmapper, bfileref)?;

    for a in bmapper.fetch_allocs() {
        child.add_mem(a, 0, kif::Perm::RWX).unwrap();
    }

    Ok(())
}

fn use_sem(is: &mut GateIStream, child: &mut dyn Child) {
    let sel: Selector = is.pop();
    let name: String = is.pop();

    let res = child.use_sem(&name, sel);
    reply_result(is, res);
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

fn handle_request(mut is: GateIStream) {
    let op: ResMngOperation = is.pop();
    let child = childs::get().child_by_id_mut(is.label() as Id).unwrap();

    match op {
        ResMngOperation::REG_SERV => reg_serv(&mut is, child),
        ResMngOperation::UNREG_SERV => unreg_serv(&mut is, child),

        ResMngOperation::OPEN_SESS => open_session(&mut is, child),
        ResMngOperation::CLOSE_SESS => close_session(&mut is, child),

        ResMngOperation::ADD_CHILD => add_child(&mut is, child),
        ResMngOperation::REM_CHILD => rem_child(&mut is, child),

        ResMngOperation::ALLOC_MEM => alloc_mem(&mut is, child),
        ResMngOperation::FREE_MEM => free_mem(&mut is, child),

        ResMngOperation::ALLOC_PE => alloc_pe(&mut is, child),
        ResMngOperation::FREE_PE => free_pe(&mut is, child),

        ResMngOperation::USE_SEM => use_sem(&mut is, child),

        _ => unreachable!(),
    }
}

fn workloop() {
    let thmng = thread::ThreadManager::get();
    let rgate = req_rgate();
    let upcall_rg = RecvGate::upcall();

    loop {
        dtu::DTUIf::sleep().ok();

        let is = rgate.fetch();
        if let Some(is) = is {
            handle_request(is);
        }

        let msg = dtu::DTUIf::fetch_msg(upcall_rg);
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

fn start_boot_mods() {
    let mut same_kmem = false;

    let mut cfgs = Vec::new();
    let moditer = boot::ModIterator::new(MODS.get().0, MODS.get().1);
    for m in moditer {
        if m.name() == "pemux" {
            continue;
        }
        // parse arguments for root
        if m.name().starts_with("root") {
            for arg in m.name().split_whitespace() {
                if arg == "samekmem" {
                    same_kmem = true;
                }
                if arg.starts_with("sem=") {
                    sems::get()
                        .add_sem(arg[4..].to_string())
                        .expect("Unable to add semaphore");
                }
            }
            continue;
        }

        let (args, daemon, cfg) =
            config::Config::new(m.name(), true).expect("Unable to parse config");
        log!(RESMNG_CFG, "Parsed config {:?}", cfg);
        cfgs.push((args, daemon, cfg));
    }

    config::check(&cfgs);

    // determine default kmem per child
    let mut total_kmem = VPE::cur()
        .kmem()
        .quota()
        .expect("Unable to determine own quota");
    let mut total_parties = cfgs.len() + 1;
    for (_, _, c) in &cfgs {
        if c.kmem() != 0 {
            total_kmem -= c.kmem();
            total_parties -= 1;
        }
    }
    let def_kmem = total_kmem / total_parties;

    let moditer = boot::ModIterator::new(MODS.get().0, MODS.get().1);
    for (id, m) in moditer.enumerate() {
        if m.name() == "pemux" || m.name().starts_with("root") {
            continue;
        }

        let (args, daemon, cfg) = cfgs.remove(0);

        // kernel memory for child
        let kmem = if cfg.kmem() == 0 && same_kmem {
            VPE::cur().kmem().clone()
        }
        else {
            let kmem_bytes = if cfg.kmem() != 0 {
                cfg.kmem()
            }
            else {
                def_kmem
            };
            VPE::cur()
                .kmem()
                .derive(kmem_bytes)
                .expect("Unable to derive new kernel memory")
        };

        let pe = pes::get()
            .find_and_alloc(VPE::cur().pe_desc())
            .expect("Unable to allocate PE");
        let mut child = OwnChild::new(id as Id, pe, args, daemon, kmem, cfg);
        if child.has_unmet_reqs() {
            DELAYED.get_mut().push(child);
        }
        else {
            start_child(&mut child, BOOT_MOD_SELS + 1 + id as Id, &m)
                .expect("Unable to start boot module");
            childs::get().add(Box::new(child));
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

    log!(RESMNG, "Boot modules:");
    MODS.set((
        mods_list.as_slice().as_ptr() as usize,
        info.mod_size as usize,
    ));
    let moditer = boot::ModIterator::new(MODS.get().0, MODS.get().1);
    for m in moditer {
        log!(RESMNG, "  {:?}", m);
    }

    let mut pes: Vec<PEDesc> = Vec::with_capacity(info.pe_count as usize);
    unsafe { pes.set_len(info.pe_count as usize) };
    mgate.read(&mut pes, off).expect("Unable to read PEs");

    let pe_sel = BOOT_MOD_SELS + 1 + info.mod_count as Selector;
    let mut user_pes = 0;
    let mut i = 0;
    log!(RESMNG, "Available PEs:");
    for pe in pes {
        log!(
            RESMNG,
            "  PE{:02}: {} {} {} KiB memory",
            i,
            pe.pe_type(),
            pe.isa(),
            pe.mem_size() / 1024
        );
        // skip kernel and our own PE
        if i > VPE::cur().pe_id() {
            pes::get().add(i as dtu::PEId, Rc::new(PE::new_bind(pe, pe_sel + i - 1)));
        }
        if i > 0 && pe.pe_type() != kif::PEType::MEM {
            user_pes += 1;
        }
        i += 1;
    }

    let mut mem_sel = BOOT_MOD_SELS + 1 + (user_pes + info.mod_count) as Selector;
    for i in 0..info.mems.len() {
        let mem = &info.mems[i];
        if mem.size() == 0 {
            continue;
        }

        memory::get().add(memory::MemMod::new(mem_sel, mem.size(), mem.reserved()));
        mem_sel += 1;
    }
    log!(RESMNG, "Memory: {:?}", memory::get());

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

    start_boot_mods();

    // ensure that there is no id overlap
    childs::get().set_next_id(info.mod_count as Id + 1);

    childs::get().start_waiting(1);

    workloop();

    log!(RESMNG, "All childs gone. Exiting.");

    0
}
