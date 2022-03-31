/*
 * Copyright (C) 2020-2022 Nils Asmussen, Barkhausen Institut
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
use m3::cell::{LazyStaticRefCell, Ref};
use m3::col::String;
use m3::com::{GateIStream, RecvGate};
use m3::errors::{Code, Error, VerboseError};
use m3::goff;
use m3::kif;
use m3::log;
use m3::reply_vmsg;
use m3::session::ResMngOperation;
use m3::tcu::ActId;
use m3::tiles::Activity;

use crate::childs::{self, Id};
use crate::sendqueue;
use crate::subsys;

static RGATE: LazyStaticRefCell<RecvGate> = LazyStaticRefCell::default();

pub fn init(rgate: RecvGate) {
    RGATE.set(rgate);
}

pub fn rgate() -> Ref<'static, RecvGate> {
    RGATE.borrow()
}

pub fn workloop<F, S>(mut func: F, mut spawn: S) -> Result<(), VerboseError>
where
    F: FnMut(),
    S: FnMut(&mut childs::OwnChild) -> Result<(), VerboseError>,
{
    let upcall_rg = RecvGate::upcall();

    loop {
        {
            let rgate = RGATE.borrow();
            if let Some(msg) = rgate.fetch() {
                let is = GateIStream::new(msg, &rgate);
                handle_request_async(is);
                subsys::start_delayed_async(&mut spawn)?;
            }
        }

        if let Some(msg) = upcall_rg.fetch() {
            childs::ChildManager::handle_upcall_async(msg);
        }

        sendqueue::check_replies();

        func();

        if thread::ready_count() > 0 {
            thread::try_yield();
        }

        if childs::borrow_mut().should_stop() {
            break;
        }

        Activity::own().sleep().ok();
    }

    if !thread::cur().is_main() {
        thread::stop();
        // just in case there is no ready thread
        m3::exit(0);
    }
    Ok(())
}

fn handle_request_async(mut is: GateIStream<'_>) {
    let op: Result<ResMngOperation, Error> = is.pop();
    let id = is.label() as Id;

    let res = match op {
        Ok(ResMngOperation::REG_SERV) => reg_serv(&mut is, id),
        Ok(ResMngOperation::UNREG_SERV) => unreg_serv(&mut is, id),

        Ok(ResMngOperation::OPEN_SESS) => open_session_async(&mut is, id),
        Ok(ResMngOperation::CLOSE_SESS) => close_session_async(&mut is, id),

        Ok(ResMngOperation::ADD_CHILD) => add_child(&mut is, id),
        Ok(ResMngOperation::REM_CHILD) => rem_child_async(&mut is, id),

        Ok(ResMngOperation::ALLOC_MEM) => alloc_mem(&mut is, id),
        Ok(ResMngOperation::FREE_MEM) => free_mem(&mut is, id),

        Ok(ResMngOperation::ALLOC_TILE) => match alloc_tile(&mut is, id) {
            // reply already done
            Ok(_) => return,
            Err(e) => Err(e),
        },
        Ok(ResMngOperation::FREE_TILE) => free_tile(&mut is, id),

        Ok(ResMngOperation::USE_RGATE) => match use_rgate(&mut is, id) {
            // reply already done
            Ok(_) => return,
            Err(e) => Err(e),
        },
        Ok(ResMngOperation::USE_SGATE) => use_sgate(&mut is, id),

        Ok(ResMngOperation::USE_SEM) => use_sem(&mut is, id),

        Ok(ResMngOperation::GET_SERIAL) => get_serial(&mut is, id),

        Ok(ResMngOperation::GET_INFO) => get_info(&mut is, id),

        _ => Err(Error::new(Code::InvArgs)),
    };

    match res {
        Err(e) => {
            let mut childs = childs::borrow_mut();
            let child = childs.child_by_id_mut(id).unwrap();
            log!(crate::LOG_DEF, "{}: {:?} failed: {}", child.name(), op, e);
            is.reply_error(e.code())
        },
        Ok(_) => is.reply_error(Code::None),
    }
    .ok(); // ignore errors; we might have removed the child in the meantime
}

fn reg_serv(is: &mut GateIStream<'_>, id: Id) -> Result<(), Error> {
    let dst_sel: Selector = is.pop()?;
    let sgate_sel: Selector = is.pop()?;
    let sessions: u32 = is.pop()?;
    let name: String = is.pop()?;

    let mut childs = childs::borrow_mut();
    let child = childs.child_by_id_mut(id).unwrap();
    child.reg_service(dst_sel, sgate_sel, name, sessions)
}

fn unreg_serv(is: &mut GateIStream<'_>, id: Id) -> Result<(), Error> {
    let sel: Selector = is.pop()?;

    let mut childs = childs::borrow_mut();
    let child = childs.child_by_id_mut(id).unwrap();
    child.unreg_service(sel)
}

fn open_session_async(is: &mut GateIStream<'_>, id: Id) -> Result<(), Error> {
    let dst_sel: Selector = is.pop()?;
    let name: String = is.pop()?;

    childs::open_session_async(id, dst_sel, &name)
}

fn close_session_async(is: &mut GateIStream<'_>, id: Id) -> Result<(), Error> {
    let sel: Selector = is.pop()?;

    childs::close_session_async(id, sel)
}

fn add_child(is: &mut GateIStream<'_>, id: Id) -> Result<(), Error> {
    let act_id: ActId = is.pop()?;
    let act_sel: Selector = is.pop()?;
    let sgate_sel: Selector = is.pop()?;
    let name: String = is.pop()?;

    childs::add_child(id, act_id, act_sel, &RGATE.borrow(), sgate_sel, name)
}

fn rem_child_async(is: &mut GateIStream<'_>, id: Id) -> Result<(), Error> {
    let act_sel: Selector = is.pop()?;

    childs::rem_child_async(id, act_sel)
}

fn alloc_mem(is: &mut GateIStream<'_>, id: Id) -> Result<(), Error> {
    let dst_sel: Selector = is.pop()?;
    let addr: goff = is.pop()?;
    let size: goff = is.pop()?;
    let perms = kif::Perm::from_bits_truncate(is.pop::<u32>()?);

    let mut childs = childs::borrow_mut();
    let child = childs.child_by_id_mut(id).unwrap();
    if addr == !0 {
        child.alloc_mem(dst_sel, size, perms)
    }
    else {
        child.alloc_mem_at(dst_sel, addr, size, perms)
    }
}

fn free_mem(is: &mut GateIStream<'_>, id: Id) -> Result<(), Error> {
    let sel: Selector = is.pop()?;

    let mut childs = childs::borrow_mut();
    let child = childs.child_by_id_mut(id).unwrap();
    child.free_mem(sel)
}

fn alloc_tile(is: &mut GateIStream<'_>, id: Id) -> Result<(), Error> {
    let dst_sel: Selector = is.pop()?;
    let desc = kif::TileDesc::new_from(is.pop()?);

    let mut childs = childs::borrow_mut();
    let child = childs.child_by_id_mut(id).unwrap();
    child
        .alloc_tile(dst_sel, desc)
        .and_then(|(id, desc)| reply_vmsg!(is, Code::None as u32, id, desc.value()))
}

fn free_tile(is: &mut GateIStream<'_>, id: Id) -> Result<(), Error> {
    let sel: Selector = is.pop()?;

    let mut childs = childs::borrow_mut();
    let child = childs.child_by_id_mut(id).unwrap();
    child.free_tile(sel)
}

fn use_rgate(is: &mut GateIStream<'_>, id: Id) -> Result<(), Error> {
    let sel: Selector = is.pop()?;
    let name: String = is.pop()?;

    let mut childs = childs::borrow_mut();
    let child = childs.child_by_id_mut(id).unwrap();
    child
        .use_rgate(&name, sel)
        .and_then(|(order, msg_order)| reply_vmsg!(is, Code::None as u32, order, msg_order))
}

fn use_sgate(is: &mut GateIStream<'_>, id: Id) -> Result<(), Error> {
    let sel: Selector = is.pop()?;
    let name: String = is.pop()?;

    let mut childs = childs::borrow_mut();
    let child = childs.child_by_id_mut(id).unwrap();
    child.use_sgate(&name, sel)
}

fn use_sem(is: &mut GateIStream<'_>, id: Id) -> Result<(), Error> {
    let sel: Selector = is.pop()?;
    let name: String = is.pop()?;

    let mut childs = childs::borrow_mut();
    let child = childs.child_by_id_mut(id).unwrap();
    child.use_sem(&name, sel)
}

fn get_serial(is: &mut GateIStream<'_>, id: Id) -> Result<(), Error> {
    let sel: Selector = is.pop()?;

    let mut childs = childs::borrow_mut();
    let child = childs.child_by_id_mut(id).unwrap();
    child.get_serial(sel)
}

fn get_info(is: &mut GateIStream<'_>, id: Id) -> Result<(), Error> {
    let act_idx: usize = is.pop()?;

    let idx = if act_idx == usize::MAX {
        None
    }
    else {
        Some(act_idx)
    };

    childs::get_info(id, idx).and_then(|info| reply_vmsg!(is, Code::None as u32, info))
}
