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

#![feature(const_vec_new)]
#![feature(core_intrinsics)]

#[macro_use]
extern crate m3;
extern crate thread;
extern crate resmng;

mod loader;

use m3::boxed::Box;
use m3::cap::Selector;
use m3::cell::{RefCell, StaticCell};
use m3::col::{String, Vec};
use m3::com::{GateIStream, MemGate, RecvGate, RGateArgs, SendGate, SGateArgs};
use m3::dtu;
use m3::errors::Error;
use m3::goff;
use m3::kif::{self, boot, PEDesc};
use m3::rc::Rc;
use m3::session::{ResMng, ResMngOperation};
use m3::util;
use m3::vpe::{VPE, VPEArgs};

use resmng::childs::{self, OwnChild, Child, Id};
use resmng::{config, memory, sendqueue, services};

//
// The kernel initializes our cap space as follows:
// +-----------+-------+-----+-----------+-------+-----------+
// | boot info | mod_0 | ... | mod_{n-1} | mem_0 | mem_{n-1} |
// +-----------+-------+-----+-----------+-------+-----------+
// ^-- FIRST_FREE_SEL
//
const BOOT_MOD_SELS: Selector = kif::FIRST_FREE_SEL;

static DELAYED: StaticCell<Vec<OwnChild>>  = StaticCell::new(Vec::new());
static MODS: StaticCell<(usize, usize)>     = StaticCell::new((0, 0));
static RGATE: StaticCell<Option<RecvGate>>  = StaticCell::new(None);

fn req_rgate() -> &'static RecvGate {
    RGATE.get().as_ref().unwrap()
}

fn reply_result(is: &mut GateIStream, res: Result<(), Error>) {
    match res {
        Err(e) => {
            log!(RESMNG, "request failed: {}", e);
            reply_vmsg!(is, e.code() as u64)
        },
        Ok(_)  => reply_vmsg!(is, 0 as u64),
    }.expect("Unable to reply");
}

fn reg_serv(is: &mut GateIStream, child: &mut Child) {
    let child_sel: Selector = is.pop();
    let dst_sel: Selector = is.pop();
    let rgate_sel: Selector = is.pop();
    let name: String = is.pop();

    let res = services::get().reg_serv(child, child_sel, dst_sel, rgate_sel, name);
    if res.is_ok() && DELAYED.get().len() > 0 {
        start_delayed();
    }
    reply_result(is, res);
}

fn unreg_serv(is: &mut GateIStream, child: &mut Child) {
    let sel: Selector = is.pop();
    let notify: bool = is.pop();

    let res = services::get().unreg_serv(child, sel, notify);
    reply_result(is, res);
}

fn open_session(is: &mut GateIStream, child: &mut Child) {
    let dst_sel: Selector = is.pop();
    let name: String = is.pop();
    let arg: u64 = is.pop();

    let res = services::get().open_session(child, dst_sel, &name, arg);
    reply_result(is, res);
}

fn close_session(is: &mut GateIStream, child: &mut Child) {
    let sel: Selector = is.pop();

    let res = services::get().close_session(child, sel);
    reply_result(is, res);
}

fn add_child(is: &mut GateIStream, child: &mut Child) {
    let vpe_sel: Selector = is.pop();
    let sgate_sel: Selector = is.pop();
    let name: String = is.pop();

    let res = child.add_child(vpe_sel, req_rgate(), sgate_sel, name);
    reply_result(is, res);
}

fn rem_child(is: &mut GateIStream, child: &mut Child) {
    let vpe_sel: Selector = is.pop();

    let res = child.rem_child(vpe_sel);
    reply_result(is, res);
}

fn alloc_mem(is: &mut GateIStream, child: &mut Child) {
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

fn free_mem(is: &mut GateIStream, child: &mut Child) {
    let sel: Selector = is.pop();

    let res = child.remove_mem(sel);
    reply_result(is, res);
}

fn start_child(child: &mut OwnChild, bsel: Selector, m: &'static boot::Mod) -> Result<(), Error> {
    let sgate = SendGate::new_with(
        SGateArgs::new(req_rgate()).credits(256).label(child.id() as u64)
    )?;
    let vpe = VPE::new_with(VPEArgs::new(child.name()).resmng(ResMng::new(sgate)))?;

    let bfile = loader::BootFile::new(bsel, m.size as usize);
    let mut bmapper = loader::BootMapper::new(vpe.sel(), bsel, vpe.pe().has_virtmem());
    let bfileref = VPE::cur().files().add(Rc::new(RefCell::new(bfile)))?;

    child.start(vpe, &mut bmapper, bfileref)?;

    for a in bmapper.fetch_allocs() {
        child.add_mem(a, 0, kif::Perm::RWX).unwrap();
    }

    Ok(())
}

fn start_delayed() {
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
    }

    if delayed.len() == 0 {
        childs::get().start_waiting(1);
    }
}

fn handle_request(mut is: GateIStream) {
    let op: ResMngOperation = is.pop();
    let child = childs::get().child_by_id_mut(is.label() as Id).unwrap();

    match op {
        ResMngOperation::REG_SERV    => reg_serv(&mut is, child),
        ResMngOperation::UNREG_SERV  => unreg_serv(&mut is, child),

        ResMngOperation::OPEN_SESS   => open_session(&mut is, child),
        ResMngOperation::CLOSE_SESS  => close_session(&mut is, child),

        ResMngOperation::ADD_CHILD   => add_child(&mut is, child),
        ResMngOperation::REM_CHILD   => rem_child(&mut is, child),

        ResMngOperation::ALLOC_MEM   => alloc_mem(&mut is, child),
        ResMngOperation::FREE_MEM    => free_mem(&mut is, child),

        _                            => unreachable!(),
    }
}

fn workloop() {
    let thmng = thread::ThreadManager::get();
    let rgate = req_rgate();
    let upcall_ep = RecvGate::upcall().ep().unwrap();

    loop {
        // we are not interested in the events here; just fetch them before the sleep
        dtu::DTU::fetch_events();
        dtu::DTU::try_sleep(true, 0).ok();

        let is = rgate.fetch();
        if let Some(is) = is {
            handle_request(is);
        }

        let msg = dtu::DTU::fetch_msg(upcall_ep);
        if let Some(msg) = msg {
            childs::get().handle_upcall(msg);
        }

        sendqueue::check_replies();

        if thmng.ready_count() > 0 {
            thmng.try_yield();
        }

        if childs::get().len() == 0 {
            break;
        }
    }

    if !thmng.cur().is_main() {
        thmng.stop();
        // just in case there is no ready thread
        m3::exit(0);
    }
}

#[no_mangle]
pub fn main() -> i32 {
    let mgate = MemGate::new_bind(BOOT_MOD_SELS);
    let mut off: goff = 0;

    let info: boot::Info = mgate.read_obj(0).expect("Unable to read boot info");
    off += util::size_of::<boot::Info>() as goff;

    let mut mods_list = vec![0u8; info.mod_size as usize];
    mgate.read(&mut mods_list, off).expect("Unable to read mods");
    off += info.mod_size;

    log!(RESMNG, "Boot modules:");
    MODS.set((mods_list.as_slice().as_ptr() as usize, info.mod_size as usize));
    let moditer = boot::ModIterator::new(MODS.get().0, MODS.get().1);
    for m in moditer {
        log!(RESMNG, "  {:?}", m);
    }

    let mut pes: Vec<PEDesc> = Vec::with_capacity(info.pe_count as usize);
    unsafe { pes.set_len(info.pe_count as usize) };
    mgate.read(&mut pes, off).expect("Unable to read PEs");

    let mut i = 0;
    log!(RESMNG, "Available PEs:");
    for pe in pes {
        log!(
            RESMNG,
            "  PE{:02}: {} {} {} KiB memory",
            i, pe.pe_type(), pe.isa(), pe.mem_size() / 1024
        );
        i += 1;
    }

    let mut mem_sel = BOOT_MOD_SELS + 1 + info.mod_count as Selector;
    for i in 0..info.mems.len() {
        let mem = &info.mems[i];
        if mem.size() == 0 {
            continue;
        }

        memory::get().add(memory::MemMod::new(mem_sel, mem.size(), mem.reserved()));
        mem_sel += 1;
    }
    log!(RESMNG, "Memory: {:?}", memory::get());

    let mut rgate = RecvGate::new_with(
        RGateArgs::new().order(12).msg_order(8)
    ).expect("Unable to create RecvGate");
    rgate.activate().expect("Unable to activate RecvGate");
    RGATE.set(Some(rgate));

    sendqueue::init();
    thread::init();
    // TODO calculate the number of threads we need (one per child?)
    for _ in 0..8 {
        thread::ThreadManager::get().add_thread(workloop as *const () as usize, 0);
    }

    let mut cfgs = Vec::new();
    let moditer = boot::ModIterator::new(MODS.get().0, MODS.get().1);
    for m in moditer {
        if m.name() == "rctmux" || m.name() == "root" {
            continue;
        }

        let (args, daemon, cfg) = config::Config::new(m.name(), true)
            .expect("Unable to parse config");
        log!(RESMNG_CFG, "Parsed config {:?}", cfg);
        cfgs.push((args, daemon, cfg));
    }

    config::check(&cfgs);

    let moditer = boot::ModIterator::new(MODS.get().0, MODS.get().1);
    for (id, m) in moditer.enumerate() {
        if m.name() == "rctmux" || m.name() == "root" {
            continue;
        }

        let (args, daemon, cfg) = cfgs.remove(0);
        let mut child = OwnChild::new(id as Id, args, daemon, cfg);
        if child.has_unmet_reqs() {
            DELAYED.get_mut().push(child);
        }
        else {
            start_child(&mut child, BOOT_MOD_SELS + 1 + id as Id, &m)
                .expect("Unable to start boot module");
            childs::get().add(Box::new(child));
        }
    }

    // ensure that there is no id overlap
    childs::get().set_next_id(info.mod_count as Id + 1);

    if DELAYED.get().len() == 0 {
        childs::get().start_waiting(1);
    }

    workloop();

    0
}