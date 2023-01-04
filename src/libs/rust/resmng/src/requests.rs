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

use m3::cell::{LazyStaticRefCell, Ref};
use m3::com::{GateIStream, RecvGate};
use m3::errors::{Code, Error, VerboseError};
use m3::log;
use m3::reply_vmsg;
use m3::session::resmng;
use m3::tiles::Activity;

use crate::childs::{ChildManager, Id};
use crate::res::Resources;
use crate::sendqueue;
use crate::subsys::{self, ChildStarter};

static RGATE: LazyStaticRefCell<RecvGate> = LazyStaticRefCell::default();

pub fn init(rgate: RecvGate) {
    RGATE.set(rgate);
}

pub fn rgate() -> Ref<'static, RecvGate> {
    RGATE.borrow()
}

pub fn workloop<F>(
    childs: &mut ChildManager,
    res: &mut Resources,
    mut func: F,
    starter: &mut dyn ChildStarter,
) -> Result<(), VerboseError>
where
    F: FnMut(&mut ChildManager, &mut Resources),
{
    let upcall_rg = RecvGate::upcall();

    loop {
        {
            let rgate = RGATE.borrow();
            if let Ok(msg) = rgate.fetch() {
                let is = GateIStream::new(msg, &rgate);
                handle_request_async(childs, res, is);
                subsys::start_delayed_async(childs, res, starter)?;
            }
        }

        if let Ok(msg) = upcall_rg.fetch() {
            childs.handle_upcall_async(res, msg);
        }

        sendqueue::check_replies(res);

        func(childs, res);

        if thread::ready_count() > 0 {
            thread::try_yield();
        }

        if childs.should_stop() {
            break;
        }

        Activity::own().sleep().ok();
    }

    if !thread::cur().is_main() {
        thread::stop();
        // just in case there is no ready thread
        Activity::own().exit(Ok(()));
    }
    Ok(())
}

fn handle_request_async(childs: &mut ChildManager, res: &mut Resources, mut is: GateIStream<'_>) {
    let op: Result<resmng::Operation, Error> = is.pop();
    let id = is.label() as Id;

    let res = match op {
        Ok(resmng::Operation::REG_SERV) => reg_serv(childs, res, &mut is, id),
        Ok(resmng::Operation::UNREG_SERV) => unreg_serv(childs, res, &mut is, id),

        Ok(resmng::Operation::OPEN_SESS) => open_session_async(childs, res, &mut is, id),
        Ok(resmng::Operation::CLOSE_SESS) => close_session_async(childs, res, &mut is, id),

        Ok(resmng::Operation::ADD_CHILD) => add_child(childs, res, &mut is, id),
        Ok(resmng::Operation::REM_CHILD) => rem_child_async(childs, res, &mut is, id),

        Ok(resmng::Operation::ALLOC_MEM) => alloc_mem(childs, res, &mut is, id),
        Ok(resmng::Operation::FREE_MEM) => free_mem(childs, res, &mut is, id),

        Ok(resmng::Operation::ALLOC_TILE) => match alloc_tile(childs, res, &mut is, id) {
            // reply already done
            Ok(_) => return,
            Err(e) => Err(e),
        },
        Ok(resmng::Operation::FREE_TILE) => free_tile(childs, res, &mut is, id),

        Ok(resmng::Operation::USE_RGATE) => match use_rgate(childs, res, &mut is, id) {
            // reply already done
            Ok(_) => return,
            Err(e) => Err(e),
        },
        Ok(resmng::Operation::USE_SGATE) => use_sgate(childs, res, &mut is, id),

        Ok(resmng::Operation::USE_SEM) => use_sem(childs, res, &mut is, id),

        Ok(resmng::Operation::USE_MOD) => use_mod(childs, res, &mut is, id),

        Ok(resmng::Operation::GET_SERIAL) => get_serial(childs, res, &mut is, id),

        Ok(resmng::Operation::GET_INFO) => get_info(childs, res, &mut is, id),

        _ => Err(Error::new(Code::InvArgs)),
    };

    match res {
        Err(e) => {
            let child = childs.child_by_id_mut(id).unwrap();
            log!(crate::LOG_DEF, "{}: {:?} failed: {}", child.name(), op, e);
            is.reply_error(e.code())
        },
        Ok(_) => is.reply_error(Code::Success),
    }
    .ok(); // ignore errors; we might have removed the child in the meantime
}

fn reg_serv(
    childs: &mut ChildManager,
    res: &mut Resources,
    is: &mut GateIStream<'_>,
    id: Id,
) -> Result<(), Error> {
    let req: resmng::RegServiceReq = is.pop()?;

    let child = childs.child_by_id_mut(id).unwrap();
    child.reg_service(res, req.dst, req.sgate, req.name, req.sessions)
}

fn unreg_serv(
    childs: &mut ChildManager,
    res: &mut Resources,
    is: &mut GateIStream<'_>,
    id: Id,
) -> Result<(), Error> {
    let req: resmng::FreeReq = is.pop()?;

    let child = childs.child_by_id_mut(id).unwrap();
    child.unreg_service(res, req.sel)
}

fn open_session_async(
    childs: &mut ChildManager,
    res: &mut Resources,
    is: &mut GateIStream<'_>,
    id: Id,
) -> Result<(), Error> {
    let req: resmng::OpenSessionReq = is.pop()?;

    let child = childs.child_by_id_mut(id).unwrap();
    child.open_session_async(res, id, req.dst, &req.name)
}

fn close_session_async(
    childs: &mut ChildManager,
    res: &mut Resources,
    is: &mut GateIStream<'_>,
    id: Id,
) -> Result<(), Error> {
    let req: resmng::FreeReq = is.pop()?;

    let child = childs.child_by_id_mut(id).unwrap();
    child.close_session_async(res, id, req.sel)
}

fn add_child(
    childs: &mut ChildManager,
    res: &mut Resources,
    is: &mut GateIStream<'_>,
    id: Id,
) -> Result<(), Error> {
    let req: resmng::AddChildReq = is.pop()?;

    childs.add_child(
        res,
        id,
        req.id,
        req.sel,
        &RGATE.borrow(),
        req.sgate,
        req.name,
    )
}

fn rem_child_async(
    childs: &mut ChildManager,
    res: &mut Resources,
    is: &mut GateIStream<'_>,
    id: Id,
) -> Result<(), Error> {
    let req: resmng::FreeReq = is.pop()?;

    childs.rem_child_async(res, id, req.sel)
}

fn alloc_mem(
    childs: &mut ChildManager,
    _res: &mut Resources,
    is: &mut GateIStream<'_>,
    id: Id,
) -> Result<(), Error> {
    let req: resmng::AllocMemReq = is.pop()?;

    let child = childs.child_by_id_mut(id).unwrap();
    child.alloc_mem(req.dst, req.size, req.perms)
}

fn free_mem(
    childs: &mut ChildManager,
    _res: &mut Resources,
    is: &mut GateIStream<'_>,
    id: Id,
) -> Result<(), Error> {
    let req: resmng::FreeReq = is.pop()?;

    let child = childs.child_by_id_mut(id).unwrap();
    child.free_mem(req.sel)
}

fn alloc_tile(
    childs: &mut ChildManager,
    res: &mut Resources,
    is: &mut GateIStream<'_>,
    id: Id,
) -> Result<(), Error> {
    let req: resmng::AllocTileReq = is.pop()?;

    let child = childs.child_by_id_mut(id).unwrap();
    child
        .alloc_tile(res, req.dst, req.desc)
        .and_then(|(id, desc)| reply_vmsg!(is, Code::Success, resmng::AllocTileReply { id, desc }))
}

fn free_tile(
    childs: &mut ChildManager,
    res: &mut Resources,
    is: &mut GateIStream<'_>,
    id: Id,
) -> Result<(), Error> {
    let req: resmng::FreeReq = is.pop()?;

    let child = childs.child_by_id_mut(id).unwrap();
    child.free_tile(res, req.sel)
}

fn use_rgate(
    childs: &mut ChildManager,
    res: &mut Resources,
    is: &mut GateIStream<'_>,
    id: Id,
) -> Result<(), Error> {
    let req: resmng::UseReq = is.pop()?;

    let child = childs.child_by_id_mut(id).unwrap();
    child
        .use_rgate(res, &req.name, req.dst)
        .and_then(|(order, msg_order)| {
            reply_vmsg!(is, Code::Success, resmng::UseRGateReply {
                order,
                msg_order
            })
        })
}

fn use_sgate(
    childs: &mut ChildManager,
    res: &mut Resources,
    is: &mut GateIStream<'_>,
    id: Id,
) -> Result<(), Error> {
    let req: resmng::UseReq = is.pop()?;

    let child = childs.child_by_id_mut(id).unwrap();
    child.use_sgate(res, &req.name, req.dst)
}

fn use_sem(
    childs: &mut ChildManager,
    res: &mut Resources,
    is: &mut GateIStream<'_>,
    id: Id,
) -> Result<(), Error> {
    let req: resmng::UseReq = is.pop()?;

    let child = childs.child_by_id_mut(id).unwrap();
    child.use_sem(res, &req.name, req.dst)
}

fn use_mod(
    childs: &mut ChildManager,
    res: &mut Resources,
    is: &mut GateIStream<'_>,
    id: Id,
) -> Result<(), Error> {
    let req: resmng::UseReq = is.pop()?;

    let child = childs.child_by_id_mut(id).unwrap();
    child.use_mod(res, &req.name, req.dst)
}

fn get_serial(
    childs: &mut ChildManager,
    _res: &mut Resources,
    is: &mut GateIStream<'_>,
    id: Id,
) -> Result<(), Error> {
    let req: resmng::GetSerialReq = is.pop()?;

    let child = childs.child_by_id_mut(id).unwrap();
    child.get_serial(req.dst)
}

fn get_info(
    childs: &mut ChildManager,
    res: &mut Resources,
    is: &mut GateIStream<'_>,
    id: Id,
) -> Result<(), Error> {
    let req: resmng::GetInfoReq = is.pop()?;

    let idx = match req.idx {
        usize::MAX => None,
        n => Some(n),
    };

    childs
        .get_info(res, id, idx)
        .and_then(|info| reply_vmsg!(is, Code::Success, info))
}
