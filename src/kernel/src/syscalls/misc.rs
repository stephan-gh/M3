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
use crate::pes::{pemng, INVAL_ID, VPE};
use crate::platform;
use crate::syscalls::{get_request, reply_success, send_reply};

#[inline(never)]
pub fn alloc_ep(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), VerboseError> {
    let req: &kif::syscalls::AllocEP = get_request(msg)?;
    let dst_sel = req.dst_sel as CapSel;
    let vpe_sel = req.vpe_sel as CapSel;
    let epid = req.epid as tcu::EpId;
    let replies = req.replies as u32;

    sysc_log!(
        vpe,
        "alloc_ep(dst={}, vpe={}, epid={}, replies={})",
        dst_sel,
        vpe_sel,
        epid,
        replies
    );

    if !vpe.obj_caps().borrow().unused(dst_sel) {
        sysc_err!(Code::InvArgs, "Selector {} already in use", dst_sel);
    }
    if replies >= tcu::AVAIL_EPS as u32 {
        sysc_err!(Code::InvArgs, "Invalid reply count ({})", replies);
    }

    let ep_count = 1 + replies;
    let dst_vpe = get_kobj!(vpe, vpe_sel, VPE).upgrade().unwrap();
    if !dst_vpe.pe().has_quota(ep_count) {
        sysc_err!(
            Code::NoSpace,
            "PE cap has insufficient EPs (have {}, need {})",
            dst_vpe.pe().eps(),
            ep_count
        );
    }

    let mut pemux = pemng::pemux(dst_vpe.pe_id());
    let epid = if epid == tcu::TOTAL_EPS {
        match pemux.find_eps(ep_count) {
            Ok(epid) => epid,
            Err(e) => sysc_err!(e.code(), "No free EP range for {} EPs", ep_count),
        }
    }
    else {
        if epid > tcu::AVAIL_EPS || epid as u32 + ep_count > tcu::AVAIL_EPS as u32 {
            sysc_err!(Code::InvArgs, "Invalid endpoint id ({}:{})", epid, ep_count);
        }
        if !pemux.eps_free(epid, ep_count) {
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
            Rc::downgrade(&dst_vpe),
            epid,
            replies,
            dst_vpe.pe(),
        )),
    );
    try_kmem_quota!(vpe.obj_caps().borrow_mut().insert_as_child(cap, vpe_sel));

    dst_vpe.pe().alloc(ep_count);
    pemux.alloc_eps(epid, ep_count);

    let mut kreply = MsgBuf::borrow_def();
    kreply.set(kif::syscalls::AllocEPReply {
        error: 0,
        ep: epid as u64,
    });
    send_reply(msg, &kreply);

    Ok(())
}

#[inline(never)]
pub fn set_pmp(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), VerboseError> {
    let req: &kif::syscalls::SetPMP = get_request(msg)?;
    let pe_sel = req.pe_sel as CapSel;
    let mgate_sel = req.mgate_sel as CapSel;
    let epid = req.epid as tcu::EpId;

    sysc_log!(
        vpe,
        "set_pmp(pe={}, mgate={}, ep={})",
        pe_sel,
        mgate_sel,
        epid
    );

    let vpe_caps = vpe.obj_caps().borrow();
    let pe = get_kobj_ref!(vpe_caps, pe_sel, PE);
    if pe.derived() {
        sysc_err!(Code::NoPerm, "Cannot set PMP EPs for derived PE objects");
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

    let kobj = vpe_caps
        .get(mgate_sel)
        .ok_or_else(|| Error::new(Code::InvArgs))?
        .get();
    match kobj {
        KObject::MGate(mg) => {
            let mut pemux = pemng::pemux(pe.pe());

            if let Err(e) = pemux.config_mem_ep(epid, INVAL_ID, &mg, mg.pe_id()) {
                sysc_err!(e.code(), "Unable to configure PMP EP");
            }

            // remember that the MemGate is activated on this EP for the case that the MemGate gets
            // revoked. If so, the EP is automatically invalidated.
            let ep = pemux.pmp_ep(epid);
            EPObject::configure(ep, &kobj);
        },
        _ => sysc_err!(Code::InvArgs, "Expected MemGate"),
    }

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn kmem_quota(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), VerboseError> {
    let req: &kif::syscalls::KMemQuota = get_request(msg)?;
    let kmem_sel = req.kmem_sel as CapSel;

    sysc_log!(vpe, "kmem_quota(kmem={})", kmem_sel);

    let vpe_caps = vpe.obj_caps().borrow();
    let kmem = get_kobj_ref!(vpe_caps, kmem_sel, KMem);

    let mut kreply = MsgBuf::borrow_def();
    kreply.set(kif::syscalls::KMemQuotaReply {
        error: 0,
        total: kmem.quota() as u64,
        amount: kmem.left() as u64,
    });
    send_reply(msg, &kreply);

    Ok(())
}

#[inline(never)]
pub fn pe_quota(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), VerboseError> {
    let req: &kif::syscalls::PEQuota = get_request(msg)?;
    let pe_sel = req.pe_sel as CapSel;

    sysc_log!(vpe, "pe_quota(pe={})", pe_sel);

    let vpe_caps = vpe.obj_caps().borrow();
    let pe = get_kobj_ref!(vpe_caps, pe_sel, PE);

    let mut kreply = MsgBuf::borrow_def();
    kreply.set(kif::syscalls::PEQuotaReply {
        error: 0,
        total: pe.quota() as u64,
        amount: pe.eps() as u64,
    });
    send_reply(msg, &kreply);

    Ok(())
}

#[inline(never)]
pub fn get_sess(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), VerboseError> {
    let req: &kif::syscalls::GetSession = get_request(msg)?;
    let dst_sel = req.dst_sel as CapSel;
    let srv_sel = req.srv_sel as CapSel;
    let vpe_sel = req.vpe_sel as CapSel;
    let sid = req.sid;

    sysc_log!(
        vpe,
        "get_sess(dst={}, srv={}, vpe={}, sid={})",
        dst_sel,
        srv_sel,
        vpe_sel,
        sid
    );

    let vpecap = get_kobj!(vpe, vpe_sel, VPE).upgrade().unwrap();
    if !vpecap.obj_caps().borrow().unused(dst_sel) {
        sysc_err!(Code::InvArgs, "Selector {} already in use", dst_sel);
    }
    if Rc::ptr_eq(vpe, &vpecap) {
        sysc_err!(Code::InvArgs, "Cannot get session for own VPE");
    }

    // get service cap
    let mut vpe_caps = vpe.obj_caps().borrow_mut();
    let srvcap = vpe_caps
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

        try_kmem_quota!(
            vpecap
                .obj_caps()
                .borrow_mut()
                .obtain(dst_sel, csess.unwrap(), true)
        );
    }
    else {
        sysc_err!(Code::InvArgs, "Unknown session id {}", sid);
    }

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn activate_async(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), VerboseError> {
    let req: &kif::syscalls::Activate = get_request(msg)?;
    let ep_sel = req.ep_sel as CapSel;
    let gate_sel = req.gate_sel as CapSel;
    let rbuf_mem = req.rbuf_mem as CapSel;
    let rbuf_off = req.rbuf_off as goff;

    sysc_log!(
        vpe,
        "activate(ep={}, gate={}, rbuf_mem={}, rbuf_off={:#x})",
        ep_sel,
        gate_sel,
        rbuf_mem,
        rbuf_off,
    );

    let ep = get_kobj!(vpe, ep_sel, EP);

    // VPE that is currently active on the endpoint
    let ep_vpe = ep.vpe().unwrap();

    let epid = ep.ep();
    let dst_pe = ep.pe_id();

    let invalidated = match ep.deconfigure(false) {
        Ok(inv) => inv,
        Err(e) => sysc_err!(e.code(), "Invalidation of EP {}:{} failed", dst_pe, epid),
    };

    let maybe_kobj = vpe
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

                let pe_id = m.pe_id();
                if let Err(e) = pemng::pemux(dst_pe).config_mem_ep(epid, ep_vpe.id(), &m, pe_id) {
                    sysc_err!(e.code(), "Unable to configure mem EP");
                }
            },

            KObject::SGate(ref s) => {
                if s.gate_ep().get_ep().is_some() {
                    sysc_err!(Code::Exists, "SendGate is already activated");
                }

                let rgate = s.rgate().clone();

                if !rgate.activated() {
                    sysc_log!(vpe, "activate: waiting for rgate {:?}", rgate);

                    let event = rgate.get_event();
                    thread::wait_for(event);

                    sysc_log!(vpe, "activate: rgate {:?} is activated", rgate);
                }

                if let Err(e) = pemng::pemux(dst_pe).config_snd_ep(epid, ep_vpe.id(), &s) {
                    sysc_err!(e.code(), "Unable to configure send EP");
                }
            },

            KObject::RGate(ref r) => {
                if r.activated() {
                    sysc_err!(Code::Exists, "RecvGate is already activated");
                }

                // determine receive buffer address
                let rbuf_addr = if platform::pe_desc(dst_pe).has_virtmem() {
                    let rbuf = get_kobj!(vpe, rbuf_mem, MGate);
                    if rbuf_off >= rbuf.size() || rbuf_off + r.size() as goff > rbuf.size() {
                        sysc_err!(Code::InvArgs, "Invalid receive buffer memory");
                    }
                    if platform::pe_desc(rbuf.pe_id()).pe_type() != kif::PEType::MEM {
                        sysc_err!(Code::InvArgs, "rbuffer not in physical memory");
                    }
                    let rbuf_phys =
                        ktcu::glob_to_phys_remote(dst_pe, rbuf.addr(), kif::PageFlags::RW).unwrap();
                    rbuf_phys + rbuf_off
                }
                else {
                    if rbuf_mem != kif::INVALID_SEL {
                        sysc_err!(Code::InvArgs, "rbuffer mem cap given for SPM PE");
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

                r.activate(ep_vpe.pe_id(), epid, rbuf_addr);

                if let Err(e) = pemng::pemux(dst_pe).config_rcv_ep(epid, ep_vpe.id(), replies, r) {
                    r.deactivate();
                    sysc_err!(e.code(), "Unable to configure recv EP");
                }
            },

            _ => sysc_err!(Code::InvArgs, "Invalid capability"),
        };

        EPObject::configure(&ep, &kobj);
    }
    else if !invalidated {
        if let Err(e) = pemng::pemux(dst_pe).invalidate_ep(ep_vpe.id(), epid, !ep.is_rgate(), true)
        {
            sysc_err!(e.code(), "Invalidation of EP {}:{} failed", dst_pe, epid);
        }
    }

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn sem_ctrl_async(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), VerboseError> {
    let req: &kif::syscalls::SemCtrl = get_request(msg)?;
    let sem_sel = req.sem_sel as CapSel;
    let op = kif::syscalls::SemOp::from(req.op);

    sysc_log!(vpe, "sem_ctrl(sem={}, op={})", sem_sel, op);

    let sem = get_kobj!(vpe, sem_sel, Sem);

    match op {
        kif::syscalls::SemOp::UP => {
            sem.up();
        },

        kif::syscalls::SemOp::DOWN => {
            let res = SemObject::down_async(&sem);
            sysc_log!(vpe, "sem_ctrl-cont(res={:?})", res);
            if let Err(e) = res {
                sysc_err!(e.code(), "Semaphore operation failed");
            }
        },

        _ => sysc_err!(Code::InvArgs, "VPEOp unsupported: {:?}", op),
    }

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn vpe_ctrl_async(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), VerboseError> {
    let req: &kif::syscalls::VPECtrl = get_request(msg)?;
    let vpe_sel = req.vpe_sel as CapSel;
    let op = kif::syscalls::VPEOp::from(req.op);
    let arg = req.arg;

    sysc_log!(
        vpe,
        "vpe_ctrl(vpe={}, op={:?}, arg={:#x})",
        vpe_sel,
        op,
        arg
    );

    let vpecap = get_kobj!(vpe, vpe_sel, VPE).upgrade().unwrap();

    match op {
        kif::syscalls::VPEOp::INIT => {
            #[cfg(target_vendor = "host")]
            ktcu::set_mem_base(vpecap.pe_id(), arg as usize);
            if let Err(e) = loader::finish_start(&vpecap) {
                sysc_err!(e.code(), "Unable to finish init");
            }
        },

        kif::syscalls::VPEOp::START => {
            if Rc::ptr_eq(&vpe, &vpecap) {
                sysc_err!(Code::InvArgs, "VPE can't start itself");
            }

            if let Err(e) = vpecap.start_app_async(Some(arg as i32)) {
                sysc_err!(e.code(), "Unable to start VPE");
            }
        },

        kif::syscalls::VPEOp::STOP => {
            let is_self = vpe_sel == kif::SEL_VPE;
            vpecap.stop_app_async(arg as i32, is_self);
            if is_self {
                ktcu::ack_msg(ktcu::KSYS_EP, msg);
                return Ok(());
            }
        },

        _ => sysc_err!(Code::InvArgs, "VPEOp unsupported: {:?}", op),
    };

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn vpe_wait_async(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), VerboseError> {
    let req: &kif::syscalls::VPEWait = get_request(msg)?;
    let count = req.vpe_count as usize;
    let event = req.event;
    let sels = &{ req.sels };

    if count > sels.len() {
        sysc_err!(Code::InvArgs, "VPE count is invalid");
    }

    sysc_log!(vpe, "vpe_wait(vpes={}, event={})", count, event);

    let mut reply_msg = kif::syscalls::VPEWaitReply {
        error: 0,
        vpe_sel: kif::INVALID_SEL as u64,
        exitcode: 0,
    };

    // In any case, check whether a VPE already exited. If event == 0, wait until that happened.
    // For event != 0, remember that we want to get notified and send an upcall on a VPE's exit.
    if let Some((sel, code)) = vpe.wait_exit_async(event, &sels[0..count]) {
        sysc_log!(vpe, "vpe_wait-cont(vpe={}, exitcode={})", sel, code);

        reply_msg.vpe_sel = sel as u64;
        reply_msg.exitcode = code as u64;
    }

    let mut reply = MsgBuf::borrow_def();
    reply.set(reply_msg);
    send_reply(msg, &reply);

    Ok(())
}

pub fn reset_stats(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), VerboseError> {
    sysc_log!(vpe, "reset_stats()",);

    for pe in platform::user_pes() {
        // ignore errors here; don't unwrap because it will do nothing on host
        pemng::pemux(pe).reset_stats().ok();
    }

    reply_success(msg);
    Ok(())
}

pub fn noop(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), VerboseError> {
    sysc_log!(vpe, "noop()",);

    reply_success(msg);
    Ok(())
}
