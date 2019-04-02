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

#![feature(core_intrinsics)]

#[macro_use]
extern crate m3;

extern crate resmng;

use m3::boxed::Box;
use m3::cap::Selector;
use m3::cell::StaticCell;
use m3::col::{String, ToString, Vec};
use m3::com::{GateIStream, RecvGate, RGateArgs, SendGate, SGateArgs};
use m3::dtu;
use m3::env;
use m3::errors::{Code, Error};
use m3::goff;
use m3::kif;
use m3::session::{ResMng, ResMngOperation};
use m3::vpe::{DefaultMapper, VPE, VPEArgs};
use m3::vfs::{VFS, OpenFlags};

use resmng::childs::Child;
use resmng::{config, childs, sendqueue, services};

const MAX_CAPS: Selector = 100000;

static BASE_SEL: StaticCell<Selector>           = StaticCell::new(0);
static RGATE: StaticCell<Option<RecvGate>>      = StaticCell::new(None);

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

fn xlate_sel(sel: Selector) -> Result<Selector, Error> {
    if sel >= MAX_CAPS {
        Err(Error::new(Code::InvArgs))
    }
    else {
        Ok(BASE_SEL.get() + sel)
    }
}

fn reg_serv(is: &mut GateIStream, child: &mut Child) -> Result<(), Error> {
    let child_sel: Selector = is.pop();
    let dst_sel: Selector = is.pop();
    let rgate_sel: Selector = is.pop();
    let name: String = is.pop();

    services::get().reg_serv(child, child_sel, dst_sel, rgate_sel, name)
}

fn unreg_serv(is: &mut GateIStream, child: &mut Child) -> Result<(), Error> {
    let sel: Selector = is.pop();
    let notify: bool = is.pop();

    services::get().unreg_serv(child, sel, notify)
}

fn open_session(is: &mut GateIStream, child: &mut Child) -> Result<(), Error> {
    let dst_sel: Selector = is.pop();
    let name: String = is.pop();
    let arg: u64 = is.pop();

    // first check our service list
    let res = services::get().open_session(child, dst_sel, &name, arg);
    match res {
        Ok(_)   => Ok(()),
        Err(_)  => {
            // if that failed, ask our resource manager
            let our_sel = xlate_sel(dst_sel)?;
            VPE::cur().resmng().open_sess(our_sel, &name, arg)?;
            child.delegate(our_sel, dst_sel)
        },
    }
}

fn close_session(is: &mut GateIStream, child: &mut Child) -> Result<(), Error> {
    let sel: Selector = is.pop();

    let res = services::get().close_session(child, sel);
    match res {
        Ok(_)  => Ok(()),
        Err(_) => {
            let our_sel = xlate_sel(sel)?;
            VPE::cur().resmng().close_sess(our_sel)
        },
    }
}

fn add_child(is: &mut GateIStream, child: &mut Child) -> Result<(), Error> {
    let vpe_sel: Selector = is.pop();
    let sgate_sel: Selector = is.pop();
    let name: String = is.pop();

    child.add_child(vpe_sel, req_rgate(), sgate_sel, name)
}

fn rem_child(is: &mut GateIStream, child: &mut Child) -> Result<(), Error> {
    let vpe_sel: Selector = is.pop();

    child.rem_child(vpe_sel)
}

fn alloc_mem(is: &mut GateIStream, child: &mut Child) -> Result<(), Error> {
    let dst_sel: Selector = is.pop();
    let addr: goff = is.pop();
    let size: usize = is.pop();
    let perms = kif::Perm::from_bits_truncate(is.pop::<u8>());

    log!(RESMNG_MEM, "{}: allocate(dst_sel={}, addr={:#x}, size={:#x}, perm={:?})",
         child.name(), dst_sel, addr, size, perms);

    // forward memory requests to our resource manager
    let our_sel = xlate_sel(dst_sel)?;
    VPE::cur().resmng().alloc_mem(our_sel, addr, size, perms)?;

    // delegate memory to our child
    child.delegate(our_sel, dst_sel)
}

fn free_mem(is: &mut GateIStream, child: &mut Child) -> Result<(), Error> {
    let sel: Selector = is.pop();

    log!(RESMNG_MEM, "{}: free(sel={})", child.name(), sel);

    let our_sel = xlate_sel(sel)?;
    VPE::cur().resmng().free_mem(our_sel)
}

fn handle_request(mut is: GateIStream) {
    let op: ResMngOperation = is.pop();
    let child = childs::get().child_by_id_mut(is.label() as childs::Id).unwrap();

    let res = match op {
        ResMngOperation::REG_SERV    => reg_serv(&mut is, child),
        ResMngOperation::UNREG_SERV  => unreg_serv(&mut is, child),

        ResMngOperation::OPEN_SESS   => open_session(&mut is, child),
        ResMngOperation::CLOSE_SESS  => close_session(&mut is, child),

        ResMngOperation::ADD_CHILD   => add_child(&mut is, child),
        ResMngOperation::REM_CHILD   => rem_child(&mut is, child),

        ResMngOperation::ALLOC_MEM   => alloc_mem(&mut is, child),
        ResMngOperation::FREE_MEM    => free_mem(&mut is, child),

        _                            => unreachable!(),
    };
    reply_result(&mut is, res);
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
    VFS::mount("/", "m3fs", "m3fs").expect("Unable to mount root file system");

    sendqueue::init();
    thread::init();
    // TODO calculate the number of threads we need (one per child?)
    for _ in 0..8 {
        thread::ThreadManager::get().add_thread(workloop as *const () as usize, 0);
    }

    let mut rgate = RecvGate::new_with(RGateArgs::new().order(12).msg_order(8))
        .expect("Unable to create RecvGate");
    rgate.activate()
        .expect("Unable to activate RecvGate");

    let sgate = SendGate::new_with(SGateArgs::new(&rgate).credits(256))
        .expect("Unable to create SendGate");
    RGATE.set(Some(rgate));

    let args = env::args().skip(1).map(|s| s.to_string()).collect::<Vec<String>>();
    let name = args[0].clone();

    let mut vpe = VPE::new_with(
        VPEArgs::new(&name).resmng(ResMng::new(sgate)).pager("pager")
    ).expect("Unable to create VPE");

    let (_, _, cfg) = config::Config::new(&args[0], false)
        .expect("Unable to parse config");
    let mut child = childs::BootChild::new(0, args, false, cfg);
    childs::get().set_next_id(1);

    vpe.mounts().add("/", VPE::cur().mounts().get_by_path("/").unwrap()).unwrap();
    vpe.obtain_mounts().unwrap();

    let file = VFS::open(&name, OpenFlags::RX)
        .expect("Unable to open executable");
    let mut mapper = DefaultMapper::new(vpe.pe().has_virtmem());

    child.start(vpe, &mut mapper, file)
        .expect("Unable to start VPE");
    childs::get().add(Box::new(child));

    childs::get().start_waiting(1);

    BASE_SEL.set(VPE::cur().alloc_sels(MAX_CAPS));

    workloop();

    0
}
