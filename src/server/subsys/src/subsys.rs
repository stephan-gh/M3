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

use m3::boxed::Box;
use m3::cap::Selector;
use m3::cell::{RefCell, StaticCell};
use m3::col::{String, ToString, Vec};
use m3::com::{GateIStream, RGateArgs, RecvGate, SGateArgs, SendGate};
use m3::env;
use m3::errors::{Code, Error};
use m3::goff;
use m3::kif;
use m3::pes::{DefaultMapper, VPEArgs, PE, VPE};
use m3::rc::Rc;
use m3::session::{ResMng, ResMngOperation};
use m3::tcu;
use m3::vfs::{OpenFlags, VFS};

use resmng::childs::{Child, Id};
use resmng::{childs, config, memory, pes, sendqueue, services};

const MAX_CAPS: Selector = 1_000_000;
const MAX_CHILDS: Selector = 20;

struct ChildCaps {
    used: u64,
}

impl ChildCaps {
    const fn new() -> Self {
        ChildCaps { used: 0 }
    }

    fn alloc(&mut self) -> Result<Id, Error> {
        for i in 0..MAX_CHILDS {
            if self.used & (1 << i) == 0 {
                self.used |= 1 << i;
                return Ok(i);
            }
        }
        Err(Error::new(Code::NoSpace))
    }

    fn free(&mut self, id: Id) {
        self.used &= !(1 << id);
    }
}

static CHILD_CAPS: StaticCell<ChildCaps> = StaticCell::new(ChildCaps::new());
static BASE_SEL: StaticCell<Selector> = StaticCell::new(0);
static RGATE: StaticCell<Option<RecvGate>> = StaticCell::new(None);

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

fn xlate_sel(id: Id, sel: Selector) -> Result<Selector, Error> {
    if sel >= MAX_CAPS / MAX_CHILDS {
        Err(Error::new(Code::InvArgs))
    }
    else {
        Ok(BASE_SEL.get() + id * (MAX_CAPS / MAX_CHILDS) + sel)
    }
}

fn reg_serv(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let child_sel: Selector = is.pop()?;
    let dst_sel: Selector = is.pop()?;
    let rgate_sel: Selector = is.pop()?;
    let name: String = is.pop()?;

    services::get().reg_serv(child, child_sel, dst_sel, rgate_sel, name)
}

fn unreg_serv(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let sel: Selector = is.pop()?;
    let notify: bool = is.pop()?;

    services::get().unreg_serv(child, sel, notify)
}

fn open_session(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let dst_sel: Selector = is.pop()?;
    let name: String = is.pop()?;

    // first check our service list
    let res = services::get().open_session(child, dst_sel, &name);
    match res {
        Ok(_) => Ok(()),
        Err(_) => {
            // if that failed, ask our resource manager
            let our_sel = xlate_sel(child.id(), dst_sel)?;
            VPE::cur().resmng().open_sess(our_sel, &name)?;

            if let Err(e) = child.delegate(our_sel, dst_sel) {
                // if that failed, close it at our parent; ignore failures here
                VPE::cur().resmng().close_sess(our_sel).ok();
                Err(e)
            }
            else {
                Ok(())
            }
        },
    }
}

fn close_session(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let sel: Selector = is.pop()?;

    let res = services::get().close_session(child, sel);
    match res {
        Ok(_) => Ok(()),
        Err(_) => {
            let our_sel = xlate_sel(child.id(), sel)?;
            VPE::cur().resmng().close_sess(our_sel)
        },
    }
}

fn add_child(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let vpe_sel: Selector = is.pop()?;
    let sgate_sel: Selector = is.pop()?;
    let name: String = is.pop()?;

    let id = CHILD_CAPS.get_mut().alloc()?;
    childs::get().set_next_id(id);
    let res = child.add_child(vpe_sel, req_rgate(), sgate_sel, name);
    if res.is_err() {
        CHILD_CAPS.get_mut().free(id);
    }
    res
}

fn rem_child(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let vpe_sel: Selector = is.pop()?;

    let id = child.rem_child(vpe_sel)?;
    CHILD_CAPS.get_mut().free(id);
    Ok(())
}

fn alloc_mem(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let dst_sel: Selector = is.pop()?;
    let addr: goff = is.pop()?;
    let size: usize = is.pop()?;
    let perms = kif::Perm::from_bits_truncate(is.pop::<u32>()?);

    log!(
        resmng::LOG_MEM,
        "{}: alloc_mem(dst_sel={}, addr={:#x}, size={:#x}, perm={:?})",
        child.name(),
        dst_sel,
        addr,
        size,
        perms
    );

    // forward memory requests to our resource manager
    let our_sel = xlate_sel(child.id(), dst_sel)?;
    VPE::cur().resmng().alloc_mem(our_sel, addr, size, perms)?;

    // delegate memory to our child
    child.delegate(our_sel, dst_sel).or_else(|e| {
        // if that failed, free it at our parent; ignore failures here
        VPE::cur().resmng().free_mem(our_sel).ok();
        Err(e)
    })
}

fn free_mem(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let sel: Selector = is.pop()?;

    log!(resmng::LOG_MEM, "{}: free_mem(sel={})", child.name(), sel);

    let our_sel = xlate_sel(child.id(), sel)?;
    VPE::cur().resmng().free_mem(our_sel)
}

fn alloc_pe(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let dst_sel: Selector = is.pop()?;
    let desc = kif::PEDesc::new_from(is.pop()?);

    log!(
        resmng::LOG_PES,
        "{}: alloc_pe(dst_sel={}, desc={:?})",
        child.name(),
        dst_sel,
        desc
    );

    let our_sel = xlate_sel(child.id(), dst_sel)?;
    let res = VPE::cur().resmng().alloc_pe(our_sel, desc);
    match res {
        Err(e) => {
            log!(resmng::LOG_DEF, "request failed: {}", e);
            reply_vmsg!(is, e.code() as u64)
        },
        Ok(desc) => {
            // delegate PE to our child
            if let Err(e) = child.delegate(our_sel, dst_sel) {
                // if that failed, free it at our parent; ignore failures here
                VPE::cur().resmng().free_pe(our_sel).ok();
                reply_vmsg!(is, e.code() as u64)
            }
            else {
                reply_vmsg!(is, 0 as u64, desc.value())
            }
        },
    }
    .expect("Unable to reply");
    Ok(())
}

fn free_pe(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let sel: Selector = is.pop()?;

    log!(resmng::LOG_PES, "{}: free_pe(sel={})", child.name(), sel);

    let our_sel = xlate_sel(child.id(), sel)?;
    VPE::cur().resmng().free_pe(our_sel)
}

fn handle_request(mut is: GateIStream) {
    let op: Result<ResMngOperation, Error> = is.pop();
    let child = childs::get()
        .child_by_id_mut(is.label() as childs::Id)
        .unwrap();

    let res = match op {
        Ok(ResMngOperation::REG_SERV) => reg_serv(&mut is, child),
        Ok(ResMngOperation::UNREG_SERV) => unreg_serv(&mut is, child),

        Ok(ResMngOperation::OPEN_SESS) => open_session(&mut is, child),
        Ok(ResMngOperation::CLOSE_SESS) => close_session(&mut is, child),

        Ok(ResMngOperation::ADD_CHILD) => add_child(&mut is, child),
        Ok(ResMngOperation::REM_CHILD) => rem_child(&mut is, child),

        Ok(ResMngOperation::ALLOC_MEM) => alloc_mem(&mut is, child),
        Ok(ResMngOperation::FREE_MEM) => free_mem(&mut is, child),

        Ok(ResMngOperation::ALLOC_PE) => match alloc_pe(&mut is, child) {
            Ok(_) => return,
            Err(e) => Err(e),
        },
        Ok(ResMngOperation::FREE_PE) => free_pe(&mut is, child),

        _ => Err(Error::new(Code::InvArgs)),
    };
    reply_result(&mut is, res);
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

#[no_mangle]
pub fn main() -> i32 {
    sendqueue::init();
    thread::init();
    // TODO calculate the number of threads we need (one per child?)
    for _ in 0..8 {
        thread::ThreadManager::get().add_thread(workloop as *const () as usize, 0);
    }

    let mut rgate = RecvGate::new_with(RGateArgs::default().order(12).msg_order(8))
        .expect("Unable to create RecvGate");
    rgate.activate().expect("Unable to activate RecvGate");

    let sgate =
        SendGate::new_with(SGateArgs::new(&rgate).credits(1)).expect("Unable to create SendGate");
    RGATE.set(Some(rgate));

    let args = env::args()
        .skip(1)
        .map(|s| s.to_string())
        .collect::<Vec<String>>();
    let name = args[0].clone();

    pes::get().add(
        0,
        PE::new(VPE::cur().pe_desc()).expect("Unable to allocate PE"),
    );

    let peid = pes::get().find_and_alloc(VPE::cur().pe_desc()).unwrap();
    let mut vpe = VPE::new_with(
        pes::get().get(peid),
        VPEArgs::new(&name).resmng(ResMng::new(sgate)),
    )
    .expect("Unable to create VPE");

    // we don't use the memory pool
    let mem_pool = Rc::new(RefCell::new(memory::MemPool::default()));

    let cfg = Rc::new(config::AppConfig::new(args, false));
    let mut child = childs::OwnChild::new(
        0,
        peid,
        cfg.args().clone(),
        false,
        VPE::cur().kmem().clone(),
        mem_pool,
        cfg,
    );
    childs::get().set_next_id(1);

    vpe.mounts()
        .add("/", VPE::cur().mounts().get_by_path("/").unwrap())
        .unwrap();
    vpe.obtain_mounts().unwrap();

    let file = VFS::open(&name, OpenFlags::RX).expect("Unable to open executable");
    let mut mapper = DefaultMapper::new(vpe.pe_desc().has_virtmem());

    let id = CHILD_CAPS
        .get_mut()
        .alloc()
        .expect("Unable to allocate child id");
    childs::get().set_next_id(id);

    child
        .start(vpe, &mut mapper, file)
        .expect("Unable to start VPE");
    childs::get().add(Box::new(child));

    childs::get().start_waiting(1);

    BASE_SEL.set(VPE::cur().alloc_sels(MAX_CAPS));

    workloop();

    log!(resmng::LOG_DEF, "Child gone. Exiting.");

    0
}
