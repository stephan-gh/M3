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
use base::errors::Code;
use base::goff;
use base::kif::{self, CapSel};
use base::rc::{Rc, Weak};
use base::tcu;
use thread;

use arch::loader::Loader;
use cap::{Capability, KObject};
use cap::{EPObject, GateObject, KMemObject, RGateObject, SemObject};
use ktcu;
use pes::VPE;
use pes::{pemng, vpemng};
use platform;
use syscalls::{get_request, reply_success, send_reply, SyscError};

#[inline(never)]
pub fn alloc_ep(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::AllocEP = get_request(msg)?;
    let dst_sel = req.dst_sel as CapSel;
    let vpe_sel = req.vpe_sel as CapSel;
    let mut epid = req.epid as tcu::EpId;
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

    let ep_count = 1 + replies;
    let dst_vpe: Weak<VPE> = get_kobj!(vpe, vpe_sel, VPE);
    let dst_vpe = dst_vpe.upgrade().unwrap();
    if !dst_vpe.pe().has_quota(ep_count) {
        sysc_err!(
            Code::NoSpace,
            "PE cap has insufficient EPs (have {}, need {})",
            dst_vpe.pe().eps(),
            ep_count
        );
    }

    let pemux = pemng::get().pemux(dst_vpe.pe_id());
    if epid == tcu::EP_COUNT {
        epid = pemux.find_eps(ep_count).map_err(|e| {
            SyscError::new(e.code(), format!("No free EP range for {} EPs", ep_count))
        })?;
    }
    else {
        if epid > tcu::EP_COUNT || epid as u32 + ep_count > tcu::EP_COUNT as u32 {
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
    }

    let vpeid = dst_vpe.id();
    vpe.obj_caps().borrow_mut().insert(Capability::new(
        dst_sel,
        KObject::EP(EPObject::new(false, vpeid, epid, replies, pemux.pe())),
    ));
    dst_vpe.pe().alloc(ep_count);
    pemux.alloc_eps(epid, ep_count);

    let kreply = kif::syscalls::AllocEPReply {
        error: 0,
        ep: epid as u64,
    };
    send_reply(msg, &kreply);
    Ok(())
}

#[inline(never)]
pub fn kmem_quota(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::KMemQuota = get_request(msg)?;
    let kmem_sel = req.kmem_sel as CapSel;

    sysc_log!(vpe, "kmem_quota(kmem={})", kmem_sel);

    let kmem: Rc<KMemObject> = get_kobj!(vpe, kmem_sel, KMEM);

    let kreply = kif::syscalls::KMemQuotaReply {
        error: 0,
        amount: kmem.left() as u64,
    };
    send_reply(msg, &kreply);
    Ok(())
}

#[inline(never)]
pub fn pe_quota(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::PEQuota = get_request(msg)?;
    let pe_sel = req.pe_sel as CapSel;

    sysc_log!(vpe, "pe_quota(pe={})", pe_sel);

    let pe = get_kobj!(vpe, pe_sel, PE);

    let kreply = kif::syscalls::PEQuotaReply {
        error: 0,
        amount: pe.eps() as u64,
    };
    send_reply(msg, &kreply);
    Ok(())
}

#[inline(never)]
pub fn get_sess(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
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

    let vpecap: Weak<VPE> = get_kobj!(vpe, vpe_sel, VPE);
    let vpecap = vpecap.upgrade().unwrap();
    if !vpecap.obj_caps().borrow().unused(dst_sel) {
        sysc_err!(Code::InvArgs, "Selector {} already in use", dst_sel);
    }

    // get service cap
    let mut vpe_caps = vpe.obj_caps().borrow_mut();
    let srvcap = vpe_caps
        .get_mut(srv_sel)
        .ok_or_else(|| SyscError::new(Code::InvArgs, "Invalid capability".to_string()))?;
    let creator = match srvcap.get() {
        KObject::Serv(s) => s.creator(),
        _ => sysc_err!(Code::InvArgs, "Expected Serv capability"),
    };

    // find root service cap
    let srv_root = srvcap.get_root();

    // walk through the childs to find the session with given id (only root cap can create sessions)
    let mut csess = srv_root.find_child(|c| match c.get() {
        KObject::Sess(s) if s.ident() == sid => true,
        _ => false,
    });
    if let Some(KObject::Sess(s)) = csess.as_mut().map(|c| c.get()) {
        if s.creator() != creator {
            sysc_err!(Code::NoPerm, "Cannot get access to foreign session");
        }

        vpecap
            .obj_caps()
            .borrow_mut()
            .obtain(dst_sel, csess.unwrap(), true);
    }
    else {
        sysc_err!(Code::InvArgs, "Unknown session id {}", sid);
    }

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn activate(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
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

    let ep: Rc<EPObject> = get_kobj!(vpe, ep_sel, EP);

    // VPE that is currently active on the endpoint
    let vpe_ref = vpemng::get().vpe(ep.vpe()).unwrap();

    let epid = ep.ep();
    let dst_pe = ep.pe_id();
    let pemux = pemng::get().pemux(dst_pe);

    let mut invalidated = false;
    if ep.has_gate() {
        // we get the gate_object that is currently active on the ep_object
        if let Some(gate_object) = &*ep.get_gate() {
            if gate_object.is_r_gate() {
                gate_object.get_r_gate().deactivate();
            }
            else if gate_object.is_s_gate() {
                // TODO deactivate?
                pemux.invalidate_ep(epid, false).map_err(|e| {
                    SyscError::new(
                        e.code(),
                        format!("Invalidation of EP {}:{} failed", dst_pe, epid),
                    )
                })?;
                invalidated = true;
            }
            // we tell the gate that it's ep is no longer valid
            gate_object.remove_ep();
        }

        // to this after gate_object (a Ref<>) is dead! Otherwise can't delete this
        // because it is already borrowed!

        // we remove the gate currently active on this EP
        ep.remove_gate();
    }

    let maybe_kobj = vpe
        .obj_caps()
        .borrow()
        .get(gate_sel)
        .map(|cap| cap.get().clone());

    if let Some(kobj) = maybe_kobj {
        // TODO check whether the gate is already activated?

        match kobj {
            KObject::MGate(_) | KObject::SGate(_) => {
                if ep.replies() != 0 {
                    sysc_err!(Code::InvArgs, "Only rgates use EP caps with reply slots");
                }
            },
            _ => {},
        }

        match kobj {
            KObject::MGate(ref m) => {
                if m.cgp().get_ep().is_some() {
                    sysc_err!(Code::Exists, "MemGate is already activated");
                }

                let pe_id = m.pe_id();
                if let Err(e) = pemux.config_mem_ep(epid, vpe_ref.id(), &m, pe_id, rbuf_off) {
                    sysc_err!(e.code(), "Unable to configure mem EP");
                }
            },

            KObject::SGate(ref s) => {
                if s.cgp().get_ep().is_some() {
                    sysc_err!(Code::Exists, "SendGate is already activated");
                }

                let rgate: Rc<RGateObject> = s.rgate().clone();

                if !rgate.activated() {
                    sysc_log!(vpe, "activate: waiting for rgate {:?}", rgate);

                    let event = rgate.get_event();
                    thread::ThreadManager::get().wait_for(event);

                    sysc_log!(vpe, "activate: rgate {:?} is activated", rgate);
                }

                if let Err(e) = pemux.config_snd_ep(epid, vpe_ref.id(), &s) {
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
                    let rbuf_addr = rbuf.addr().raw();
                    rbuf_addr + rbuf_off
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

                r.activate(vpe_ref.pe_id(), epid, rbuf_addr);

                if let Err(e) = pemux.config_rcv_ep(epid, vpe_ref.id(), replies, r) {
                    r.deactivate();
                    sysc_err!(e.code(), "Unable to configure recv EP");
                }
            },

            _ => {
                klog!(DEF, "caps={:?}", vpe.obj_caps());
                sysc_err!(Code::InvArgs, "Invalid capability")
            },
        };

        // create a gate object from the kobj
        let go = match kobj {
            KObject::MGate(g) => GateObject::MGate(Rc::downgrade(&g)),
            KObject::RGate(g) => GateObject::RGate(Rc::downgrade(&g)),
            KObject::SGate(g) => GateObject::SGate(Rc::downgrade(&g)),
            _ => sysc_err!(Code::InvArgs, "Invalid capability"),
        };
        // we tell the gate object its gate object
        go.set_ep(&ep);
        // we tell the endpoint its current gate object
        ep.set_gate(go);
    }
    else if !invalidated {
        pemux.invalidate_ep(epid, true).map_err(|e| {
            SyscError::new(
                e.code(),
                format!("Invalidation of EP {}:{} failed", dst_pe, epid),
            )
        })?;
    }

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn sem_ctrl(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
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
            let res = SemObject::down(&sem);
            sysc_log!(vpe, "sem_ctrl-cont(res={:?})", res);
            if let Err(e) = res {
                sysc_err!(e.code(), "Semaphore operation failed");
            }
        },

        _ => panic!("VPEOp unsupported: {:?}", op),
    }

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn vpe_ctrl(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
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

    let vpecap: Weak<VPE> = get_kobj!(vpe, vpe_sel, VPE);
    let vpecap = vpecap.upgrade().unwrap();

    match op {
        kif::syscalls::VPEOp::INIT => {
            vpecap.set_mem_base(arg as goff);
            Loader::get().finish_start(&vpecap)
                .map_err(|e| SyscError::new(e.code(), "Unable to finish init".to_string()))?;
        },

        kif::syscalls::VPEOp::START => {
            if Rc::ptr_eq(&vpe, &vpecap) {
                sysc_err!(Code::InvArgs, "VPE can't start itself");
            }

            VPE::start_app(&vpecap, Some(arg as i32))
                .map_err(|e| SyscError::new(e.code(), "Unable to start VPE".to_string()))?;
        },

        kif::syscalls::VPEOp::STOP => {
            let is_self = vpe_sel == kif::SEL_VPE;
            VPE::stop_app(&vpecap, arg as i32, is_self);
            if is_self {
                ktcu::ack_msg(ktcu::KSYS_EP, msg);
                return Ok(());
            }
        },

        _ => panic!("VPEOp unsupported: {:?}", op),
    };

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn vpe_wait(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::VPEWait = get_request(msg)?;
    let count = req.vpe_count as usize;
    let event = req.event;
    let sels = &{ req.sels };

    if count == 0 || count > sels.len() {
        sysc_err!(Code::InvArgs, "VPE count is invalid");
    }

    sysc_log!(vpe, "vpe_wait(vpes={}, event={})", count, event);

    let mut reply = kif::syscalls::VPEWaitReply {
        error: 0,
        vpe_sel: kif::INVALID_SEL as u64,
        exitcode: 0,
    };

    if event != 0 {
        // early-reply to the application; we'll notify it later via upcall
        send_reply(msg, &reply);
    }

    // TODO copy the message to somewhere else to ensure that we can still access it after reply
    if !VPE::wait_exit_async(vpe, sels, &mut reply) && event == 0 {
        sysc_err!(Code::InvArgs, "Sync wait while async wait in progress");
    }

    if reply.vpe_sel != kif::INVALID_SEL as u64 {
        sysc_log!(
            vpe,
            "vpe_wait-cont(vpe={}, exitcode={})",
            { reply.vpe_sel },
            { reply.exitcode }
        );

        if event != 0 {
            vpe.upcall_vpewait(event, &reply);
        }
        else {
            send_reply(msg, &reply);
        }
    }

    Ok(())
}

pub fn noop(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    sysc_log!(vpe, "noop()",);

    reply_success(msg);
    Ok(())
}
