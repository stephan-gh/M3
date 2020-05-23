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
use m3::cell::LazyStaticCell;
use m3::col::String;
use m3::com::{GateIStream, RecvGate};
use m3::errors::{Code, Error};
use m3::goff;
use m3::kif;
use m3::session::ResMngOperation;
use m3::tcu;

use childs::{self, Child, Id};
use sendqueue;
use subsys;

static RGATE: LazyStaticCell<RecvGate> = LazyStaticCell::default();

pub fn init(rgate: RecvGate) {
    RGATE.set(rgate);
}

pub fn rgate() -> &'static RecvGate {
    &RGATE
}

pub fn workloop<F, S>(mut func: F, mut spawn: S) -> Result<(), Error>
where
    F: FnMut(),
    S: FnMut(&mut childs::OwnChild) -> Result<(), Error>,
{
    let thmng = thread::ThreadManager::get();
    let upcall_rg = RecvGate::upcall();

    loop {
        tcu::TCUIf::sleep().ok();

        let is = RGATE.fetch();
        if let Some(is) = is {
            handle_request(is);
            subsys::start_delayed(&mut spawn)?;
        }

        let msg = tcu::TCUIf::fetch_msg(upcall_rg);
        if let Some(msg) = msg {
            childs::get().handle_upcall(msg);
        }

        sendqueue::check_replies();

        func();

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
    Ok(())
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

    match res {
        Err(e) => {
            log!(crate::LOG_DEF, "{}: {:?} failed: {}", child.name(), op, e);
            reply_vmsg!(is, e.code() as u64)
        },
        Ok(_) => reply_vmsg!(is, 0 as u64),
    }
    .expect("Unable to reply");
}

fn reg_serv(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let dst_sel: Selector = is.pop()?;
    let sgate_sel: Selector = is.pop()?;
    let name: String = is.pop()?;

    child.reg_service(dst_sel, sgate_sel, name)
}

fn unreg_serv(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let sel: Selector = is.pop()?;
    let notify: bool = is.pop()?;

    child.unreg_service(sel, notify)
}

fn open_session(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let dst_sel: Selector = is.pop()?;
    let name: String = is.pop()?;

    child.open_session(dst_sel, &name)
}

fn close_session(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let sel: Selector = is.pop()?;

    child.close_session(sel)
}

fn add_child(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let vpe_sel: Selector = is.pop()?;
    let sgate_sel: Selector = is.pop()?;
    let name: String = is.pop()?;

    child.add_child(vpe_sel, &RGATE, sgate_sel, name)
}

fn rem_child(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let vpe_sel: Selector = is.pop()?;

    child.rem_child(vpe_sel)
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
