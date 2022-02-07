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

use base::cfg;
use base::col::ToString;
use base::errors::{Code, Error, VerboseError};
use base::goff;
use base::kif::{self, CapSel};
use base::mem::MsgBuf;
use base::rc::Rc;
use base::tcu;

use crate::arch::loader;
use crate::cap::{Capability, KObject};
use crate::cap::{EPObject, SemObject};
use crate::ktcu;
use crate::platform;
use crate::syscalls::{get_request, reply_success, send_reply};
use crate::tiles::{tilemng, Activity, TileMux, INVAL_ID};

#[inline(never)]
pub fn alloc_ep(act: &Rc<Activity>, msg: &'static tcu::Message) -> Result<(), VerboseError> {
    let req: &kif::syscalls::AllocEP = get_request(msg)?;
    let dst_sel = req.dst_sel as CapSel;
    let act_sel = req.act_sel as CapSel;
    let epid = req.epid as tcu::EpId;
    let replies = req.replies as u32;

    sysc_log!(
        act,
        "alloc_ep(dst={}, act={}, epid={}, replies={})",
        dst_sel,
        act_sel,
        epid,
        replies
    );

    if !act.obj_caps().borrow().unused(dst_sel) {
        sysc_err!(Code::InvArgs, "Selector {} already in use", dst_sel);
    }
    if replies >= tcu::AVAIL_EPS as u32 {
        sysc_err!(Code::InvArgs, "Invalid reply count ({})", replies);
    }

    let ep_count = 1 + replies;
    let dst_act = get_kobj!(act, act_sel, Activity).upgrade().unwrap();
    if !dst_act.tile().has_quota(ep_count) {
        sysc_err!(
            Code::NoSpace,
            "Tile cap has insufficient EPs (have {}, need {})",
            dst_act.tile().ep_quota().left(),
            ep_count
        );
    }

    let mut tilemux = tilemng::tilemux(dst_act.tile_id());
    let epid = if epid == tcu::TOTAL_EPS {
        match tilemux.find_eps(ep_count) {
            Ok(epid) => epid,
            Err(e) => sysc_err!(e.code(), "No free EP range for {} EPs", ep_count),
        }
    }
    else {
        if epid > tcu::AVAIL_EPS || epid as u32 + ep_count > tcu::AVAIL_EPS as u32 {
            sysc_err!(Code::InvArgs, "Invalid endpoint id ({}:{})", epid, ep_count);
        }
        if !tilemux.eps_free(epid, ep_count) {
            sysc_err!(
                Code::InvArgs,
                "Endpoints {}..{} not free",
                epid,
                epid as u32 + ep_count - 1
            );
        }
        epid
    };

    let cap = Capability::new(
        dst_sel,
        KObject::EP(EPObject::new(
            false,
            Rc::downgrade(&dst_act),
            epid,
            replies,
            dst_act.tile(),
        )),
    );
    try_kmem_quota!(act.obj_caps().borrow_mut().insert_as_child(cap, act_sel));

    dst_act.tile().alloc(ep_count);
    tilemux.alloc_eps(epid, ep_count);

    let mut kreply = MsgBuf::borrow_def();
    kreply.set(kif::syscalls::AllocEPReply {
        error: 0,
        ep: epid as u64,
    });
    send_reply(msg, &kreply);

    Ok(())
}

#[inline(never)]
pub fn set_pmp(act: &Rc<Activity>, msg: &'static tcu::Message) -> Result<(), VerboseError> {
    let req: &kif::syscalls::SetPMP = get_request(msg)?;
    let tile_sel = req.tile_sel as CapSel;
    let mgate_sel = req.mgate_sel as CapSel;
    let epid = req.epid as tcu::EpId;

    sysc_log!(
        act,
        "set_pmp(tile={}, mgate={}, ep={})",
        tile_sel,
        mgate_sel,
        epid
    );

    let act_caps = act.obj_caps().borrow();
    let tile = get_kobj_ref!(act_caps, tile_sel, Tile);
    if tile.derived() {
        sysc_err!(Code::NoPerm, "Cannot set PMP EPs for derived tile objects");
    }

    // for host: just pretend that we installed it
    if tcu::PMEM_PROT_EPS == 0 {
        reply_success(msg);
        return Ok(());
    }
    if epid < 1 || epid >= tcu::PMEM_PROT_EPS as tcu::EpId {
        sysc_err!(
            Code::InvArgs,
            "Only EPs 1..{} can be used for set_pmp",
            tcu::PMEM_PROT_EPS
        );
    }

    let kobj = act_caps
        .get(mgate_sel)
        .ok_or_else(|| Error::new(Code::InvArgs))?
        .get();
    match kobj {
        KObject::MGate(mg) => {
            let mut tilemux = tilemng::tilemux(tile.tile());

            if let Err(e) = tilemux.config_mem_ep(epid, INVAL_ID, &mg, mg.tile_id()) {
                sysc_err!(e.code(), "Unable to configure PMP EP");
            }

            // remember that the MemGate is activated on this EP for the case that the MemGate gets
            // revoked. If so, the EP is automatically invalidated.
            let ep = tilemux.pmp_ep(epid);
            EPObject::configure(ep, &kobj);
        },
        _ => sysc_err!(Code::InvArgs, "Expected MemGate"),
    }

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn mgate_region(act: &Rc<Activity>, msg: &'static tcu::Message) -> Result<(), VerboseError> {
    let req: &kif::syscalls::MGateRegion = get_request(msg)?;
    let mgate_sel = req.mgate_sel as CapSel;

    sysc_log!(act, "mgate_addr(mgate={})", mgate_sel);

    let act_caps = act.obj_caps().borrow();
    let mgate = get_kobj_ref!(act_caps, mgate_sel, MGate);

    let mut kreply = MsgBuf::borrow_def();
    kreply.set(kif::syscalls::MGateRegionReply {
        error: 0,
        global: mgate.addr().raw() as u64,
        size: mgate.size() as u64,
    });
    send_reply(msg, &kreply);

    Ok(())
}

#[inline(never)]
pub fn kmem_quota(act: &Rc<Activity>, msg: &'static tcu::Message) -> Result<(), VerboseError> {
    let req: &kif::syscalls::KMemQuota = get_request(msg)?;
    let kmem_sel = req.kmem_sel as CapSel;

    sysc_log!(act, "kmem_quota(kmem={})", kmem_sel);

    let act_caps = act.obj_caps().borrow();
    let kmem = get_kobj_ref!(act_caps, kmem_sel, KMem);

    let mut kreply = MsgBuf::borrow_def();
    kreply.set(kif::syscalls::KMemQuotaReply {
        error: 0,
        id: kmem.id() as u64,
        total: kmem.quota() as u64,
        left: kmem.left() as u64,
    });
    send_reply(msg, &kreply);

    Ok(())
}

#[inline(never)]
pub fn tile_quota_async(
    act: &Rc<Activity>,
    msg: &'static tcu::Message,
) -> Result<(), VerboseError> {
    let req: &kif::syscalls::TileQuota = get_request(msg)?;
    let tile_sel = req.tile_sel as CapSel;

    sysc_log!(act, "tile_quota(tile={})", tile_sel);

    let act_caps = act.obj_caps().borrow();
    let tile = get_kobj_ref!(act_caps, tile_sel, Tile);

    let (time, pts) = TileMux::get_quota_async(
        tilemng::tilemux(tile.tile()),
        tile.time_quota_id(),
        tile.pt_quota_id(),
    )
    .map_err(|e| {
        VerboseError::new(
            e.code(),
            base::format!(
                "Unable to get quota for time={}, pts={}",
                tile.time_quota_id(),
                tile.pt_quota_id()
            ),
        )
    })?;

    let mut kreply = MsgBuf::borrow_def();
    kreply.set(kif::syscalls::TileQuotaReply {
        error: 0,
        eps_id: tile.ep_quota().id() as u64,
        eps_total: tile.ep_quota().total() as u64,
        eps_left: tile.ep_quota().left() as u64,
        time_id: time.id(),
        time_total: time.total(),
        time_left: time.left(),
        pts_id: pts.id(),
        pts_total: pts.total() as u64,
        pts_left: pts.left() as u64,
    });
    send_reply(msg, &kreply);

    Ok(())
}

#[inline(never)]
pub fn tile_set_quota_async(
    act: &Rc<Activity>,
    msg: &'static tcu::Message,
) -> Result<(), VerboseError> {
    let req: &kif::syscalls::TileSetQuota = get_request(msg)?;
    let tile_sel = req.tile_sel as CapSel;
    let time = req.time;
    let pts = req.pts;

    sysc_log!(
        act,
        "tile_set_quota(tile={}, time={}, pts={})",
        tile_sel,
        time,
        pts
    );

    let act_caps = act.obj_caps().borrow();
    let tile = get_kobj_ref!(act_caps, tile_sel, Tile);

    if tile.derived() {
        sysc_err!(
            Code::NoPerm,
            "Cannot set tile quota with derived tile capability"
        );
    }
    if tile.activities() > 1 {
        sysc_err!(
            Code::InvArgs,
            "Cannot set tile quota with more than one Activity on the tile"
        );
    }

    let tilemux = tilemng::tilemux(tile.tile());
    // the root tile object has always the same id for the time quota and the pts quota
    TileMux::set_quota_async(tilemux, tile.time_quota_id(), time, pts)?;

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn get_sess(act: &Rc<Activity>, msg: &'static tcu::Message) -> Result<(), VerboseError> {
    let req: &kif::syscalls::GetSession = get_request(msg)?;
    let dst_sel = req.dst_sel as CapSel;
    let srv_sel = req.srv_sel as CapSel;
    let act_sel = req.act_sel as CapSel;
    let sid = req.sid;

    sysc_log!(
        act,
        "get_sess(dst={}, srv={}, act={}, sid={})",
        dst_sel,
        srv_sel,
        act_sel,
        sid
    );

    let actcap = get_kobj!(act, act_sel, Activity).upgrade().unwrap();
    if !actcap.obj_caps().borrow().unused(dst_sel) {
        sysc_err!(Code::InvArgs, "Selector {} already in use", dst_sel);
    }
    if Rc::ptr_eq(act, &actcap) {
        sysc_err!(Code::InvArgs, "Cannot get session for own Activity");
    }

    // get service cap
    let mut act_caps = act.obj_caps().borrow_mut();
    let srvcap = act_caps
        .get_mut(srv_sel)
        .ok_or_else(|| VerboseError::new(Code::InvArgs, "Invalid capability".to_string()))?;
    let creator = as_obj!(srvcap.get(), Serv).creator();

    // find root service cap
    let srv_root = srvcap.get_root();

    // walk through the childs to find the session with given id (only root cap can create sessions)
    let mut csess =
        srv_root.find_child(|c| matches!(c.get(), KObject::Sess(s) if s.ident() == sid));
    if let Some(KObject::Sess(s)) = csess.as_mut().map(|c| c.get()) {
        if s.creator() != creator {
            sysc_err!(Code::NoPerm, "Cannot get access to foreign session");
        }

        try_kmem_quota!(actcap
            .obj_caps()
            .borrow_mut()
            .obtain(dst_sel, csess.unwrap(), true));
    }
    else {
        sysc_err!(Code::InvArgs, "Unknown session id {}", sid);
    }

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn activate_async(act: &Rc<Activity>, msg: &'static tcu::Message) -> Result<(), VerboseError> {
    let req: &kif::syscalls::Activate = get_request(msg)?;
    let ep_sel = req.ep_sel as CapSel;
    let gate_sel = req.gate_sel as CapSel;
    let rbuf_mem = req.rbuf_mem as CapSel;
    let rbuf_off = req.rbuf_off as goff;

    sysc_log!(
        act,
        "activate(ep={}, gate={}, rbuf_mem={}, rbuf_off={:#x})",
        ep_sel,
        gate_sel,
        rbuf_mem,
        rbuf_off,
    );

    let ep = get_kobj!(act, ep_sel, EP);

    // activity that is currently active on the endpoint
    let ep_act = ep.activity().unwrap();

    let epid = ep.ep();
    let dst_tile = ep.tile_id();

    let invalidated = match ep.deconfigure(false) {
        Ok(inv) => inv,
        Err(e) => sysc_err!(e.code(), "Invalidation of EP {}:{} failed", dst_tile, epid),
    };

    let maybe_kobj = act
        .obj_caps()
        .borrow()
        .get(gate_sel)
        .map(|cap| cap.get().clone());

    if let Some(kobj) = maybe_kobj {
        match kobj {
            KObject::MGate(_) | KObject::SGate(_) => {
                if ep.replies() != 0 {
                    sysc_err!(Code::InvArgs, "Only rgates use EP caps with reply slots");
                }
                if rbuf_off != 0 || rbuf_mem != kif::INVALID_SEL {
                    sysc_err!(Code::InvArgs, "Only rgates specify receive buffers");
                }
            },
            _ => {},
        }

        match kobj {
            KObject::MGate(ref m) => {
                if m.gate_ep().get_ep().is_some() {
                    sysc_err!(Code::Exists, "MemGate is already activated");
                }

                let tile_id = m.tile_id();
                if let Err(e) =
                    tilemng::tilemux(dst_tile).config_mem_ep(epid, ep_act.id(), &m, tile_id)
                {
                    sysc_err!(e.code(), "Unable to configure mem EP");
                }
            },

            KObject::SGate(ref s) => {
                if s.gate_ep().get_ep().is_some() {
                    sysc_err!(Code::Exists, "SendGate is already activated");
                }

                let rgate = s.rgate().clone();

                if !rgate.activated() {
                    sysc_log!(act, "activate: waiting for rgate {:?}", rgate);

                    let event = rgate.get_event();
                    thread::wait_for(event);

                    sysc_log!(act, "activate: rgate {:?} is activated", rgate);
                }

                if let Err(e) = tilemng::tilemux(dst_tile).config_snd_ep(epid, ep_act.id(), &s) {
                    sysc_err!(e.code(), "Unable to configure send EP");
                }
            },

            KObject::RGate(ref r) => {
                if r.activated() {
                    sysc_err!(Code::Exists, "RecvGate is already activated");
                }

                // determine receive buffer address
                let rbuf_addr = if platform::tile_desc(dst_tile).has_virtmem()
                    && epid == ep_act.eps_start() + tcu::PG_REP_OFF
                {
                    // special case for activating the pager reply rgate: there is no way to get a
                    // memory capability to the standard receive buffer. thus, we just determine the
                    // physical address here and remove the choice for the user.
                    ep_act.rbuf_addr()
                        + cfg::SYSC_RBUF_SIZE as goff
                        + cfg::UPCALL_RBUF_SIZE as goff
                        + cfg::DEF_RBUF_SIZE as goff
                }
                else if platform::tile_desc(dst_tile).has_virtmem() {
                    let rbuf = get_kobj!(act, rbuf_mem, MGate);
                    if rbuf_off >= rbuf.size() || rbuf_off + r.size() as goff > rbuf.size() {
                        sysc_err!(Code::InvArgs, "Invalid receive buffer memory");
                    }
                    if platform::tile_desc(rbuf.tile_id()).tile_type() != kif::TileType::MEM {
                        sysc_err!(Code::InvArgs, "rbuffer not in physical memory");
                    }
                    let rbuf_phys =
                        ktcu::glob_to_phys_remote(dst_tile, rbuf.addr(), kif::PageFlags::RW)
                            .unwrap();
                    rbuf_phys + rbuf_off
                }
                else {
                    if rbuf_mem != kif::INVALID_SEL {
                        sysc_err!(Code::InvArgs, "rbuffer mem cap given for SPM tile");
                    }
                    rbuf_off
                };

                let replies = if ep.replies() > 0 {
                    let slots = 1 << (r.order() - r.msg_order());
                    if ep.replies() != slots {
                        sysc_err!(
                            Code::InvArgs,
                            "EP cap has {} reply slots, need {}",
                            ep.replies(),
                            slots
                        );
                    }
                    Some(epid + 1)
                }
                else {
                    None
                };

                r.activate(ep_act.tile_id(), epid, rbuf_addr);

                if let Err(e) =
                    tilemng::tilemux(dst_tile).config_rcv_ep(epid, ep_act.id(), replies, r)
                {
                    r.deactivate();
                    sysc_err!(e.code(), "Unable to configure recv EP");
                }
            },

            _ => sysc_err!(Code::InvArgs, "Invalid capability"),
        };

        EPObject::configure(&ep, &kobj);
    }
    else if !invalidated {
        if let Err(e) =
            tilemng::tilemux(dst_tile).invalidate_ep(ep_act.id(), epid, !ep.is_rgate(), true)
        {
            sysc_err!(e.code(), "Invalidation of EP {}:{} failed", dst_tile, epid);
        }
    }

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn sem_ctrl_async(act: &Rc<Activity>, msg: &'static tcu::Message) -> Result<(), VerboseError> {
    let req: &kif::syscalls::SemCtrl = get_request(msg)?;
    let sem_sel = req.sem_sel as CapSel;
    let op = kif::syscalls::SemOp::from(req.op);

    sysc_log!(act, "sem_ctrl(sem={}, op={})", sem_sel, op);

    let sem = get_kobj!(act, sem_sel, Sem);

    match op {
        kif::syscalls::SemOp::UP => {
            sem.up();
        },

        kif::syscalls::SemOp::DOWN => {
            let res = SemObject::down_async(&sem);
            sysc_log!(act, "sem_ctrl-cont(res={:?})", res);
            if let Err(e) = res {
                sysc_err!(e.code(), "Semaphore operation failed");
            }
        },

        _ => sysc_err!(Code::InvArgs, "ActivityOp unsupported: {:?}", op),
    }

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn activity_ctrl_async(
    act: &Rc<Activity>,
    msg: &'static tcu::Message,
) -> Result<(), VerboseError> {
    let req: &kif::syscalls::ActivityCtrl = get_request(msg)?;
    let act_sel = req.act_sel as CapSel;
    let op = kif::syscalls::ActivityOp::from(req.op);
    let arg = req.arg;

    sysc_log!(
        act,
        "activity_ctrl(act={}, op={:?}, arg={:#x})",
        act_sel,
        op,
        arg
    );

    let actcap = get_kobj!(act, act_sel, Activity).upgrade().unwrap();

    match op {
        kif::syscalls::ActivityOp::INIT => {
            #[cfg(target_vendor = "host")]
            ktcu::set_mem_base(actcap.tile_id(), arg as usize);
            if let Err(e) = loader::finish_start(&actcap) {
                sysc_err!(e.code(), "Unable to finish init");
            }
        },

        kif::syscalls::ActivityOp::START => {
            if Rc::ptr_eq(&act, &actcap) {
                sysc_err!(Code::InvArgs, "Activity can't start itself");
            }

            if let Err(e) = actcap.start_app_async(Some(arg as i32)) {
                sysc_err!(e.code(), "Unable to start Activity");
            }
        },

        kif::syscalls::ActivityOp::STOP => {
            let is_self = act_sel == kif::SEL_ACT;
            actcap.stop_app_async(arg as i32, is_self);
            if is_self {
                ktcu::ack_msg(ktcu::KSYS_EP, msg);
                return Ok(());
            }
        },

        _ => sysc_err!(Code::InvArgs, "ActivityOp unsupported: {:?}", op),
    };

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn activity_wait_async(
    act: &Rc<Activity>,
    msg: &'static tcu::Message,
) -> Result<(), VerboseError> {
    let req: &kif::syscalls::ActivityWait = get_request(msg)?;
    let count = req.act_count as usize;
    let event = req.event;
    let sels = &{ req.sels };

    if count > sels.len() {
        sysc_err!(Code::InvArgs, "Activity count is invalid");
    }

    sysc_log!(act, "activity_wait(activities={}, event={})", count, event);

    let mut reply_msg = kif::syscalls::ActivityWaitReply {
        error: 0,
        act_sel: kif::INVALID_SEL as u64,
        exitcode: 0,
    };

    // In any case, check whether a activity already exited. If event == 0, wait until that happened.
    // For event != 0, remember that we want to get notified and send an upcall on a activity's exit.
    if let Some((sel, code)) = act.wait_exit_async(event, &sels[0..count]) {
        sysc_log!(act, "act_wait-cont(act={}, exitcode={})", sel, code);

        reply_msg.act_sel = sel as u64;
        reply_msg.exitcode = code as u64;
    }

    let mut reply = MsgBuf::borrow_def();
    reply.set(reply_msg);
    send_reply(msg, &reply);

    Ok(())
}

pub fn reset_stats(act: &Rc<Activity>, msg: &'static tcu::Message) -> Result<(), VerboseError> {
    sysc_log!(act, "reset_stats()",);

    for tile in platform::user_tiles() {
        // ignore errors here; don't unwrap because it will do nothing on host
        tilemng::tilemux(tile).reset_stats().ok();
    }

    reply_success(msg);
    Ok(())
}

pub fn noop(act: &Rc<Activity>, msg: &'static tcu::Message) -> Result<(), VerboseError> {
    sysc_log!(act, "noop()",);

    reply_success(msg);
    Ok(())
}
