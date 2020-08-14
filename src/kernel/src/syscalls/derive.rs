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
use base::kif::{self, CapRngDesc, CapSel, CapType};
use base::mem::GlobAddr;
use base::rc::Rc;
use base::tcu;
use base::util;

use crate::cap::{Capability, KObject};
use crate::cap::{KMemObject, MGateObject, PEObject, ServObject};
use crate::com::Service;
use crate::mem;
use crate::pes::VPE;
use crate::syscalls::{get_request, reply_success, SyscError};

#[inline(never)]
pub fn derive_pe(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::DerivePE = get_request(msg)?;
    let pe_sel = req.pe_sel as CapSel;
    let dst_sel = req.dst_sel as CapSel;
    let eps = req.eps as u32;

    sysc_log!(
        vpe,
        "derive_pe(pe={}, dst={}, eps={})",
        pe_sel,
        dst_sel,
        eps
    );

    if !vpe.obj_caps().borrow().unused(dst_sel) {
        sysc_err!(Code::InvArgs, "Selector {} already in use", dst_sel);
    }

    let pe = get_kobj!(vpe, pe_sel, PE);
    if !pe.has_quota(eps) {
        sysc_err!(Code::NoSpace, "Insufficient EPs");
    }

    let cap = Capability::new(dst_sel, KObject::PE(PEObject::new(pe.pe(), eps)));
    try_kmem_quota!(vpe.obj_caps().borrow_mut().insert_as_child(cap, pe_sel));
    pe.alloc(eps);

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn derive_kmem(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::DeriveKMem = get_request(msg)?;
    let kmem_sel = req.kmem_sel as CapSel;
    let dst_sel = req.dst_sel as CapSel;
    let quota = req.quota as usize;

    sysc_log!(
        vpe,
        "derive_kmem(kmem={}, dst={}, quota={:#x})",
        kmem_sel,
        dst_sel,
        quota
    );

    if !vpe.obj_caps().borrow().unused(dst_sel) {
        sysc_err!(Code::InvArgs, "Selector {} already in use", dst_sel);
    }

    let kmem = get_kobj!(vpe, kmem_sel, KMem);
    if !kmem.has_quota(quota) {
        sysc_err!(Code::NoSpace, "Insufficient quota");
    }

    let cap = Capability::new(dst_sel, KObject::KMem(KMemObject::new(quota)));
    try_kmem_quota!(vpe.obj_caps().borrow_mut().insert_as_child(cap, kmem_sel));
    assert!(kmem.alloc(vpe, kmem_sel, quota));

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn derive_mem(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::DeriveMem = get_request(msg)?;
    let vpe_sel = req.vpe_sel as CapSel;
    let dst_sel = req.dst_sel as CapSel;
    let src_sel = req.src_sel as CapSel;
    let offset = req.offset as goff;
    let size = req.size as goff;
    let perms = kif::Perm::from_bits_truncate(req.perms as u32);

    sysc_log!(
        vpe,
        "derive_mem(vpe={}, src={}, dst={}, size={:#x}, offset={:#x}, perms={:?})",
        vpe_sel,
        src_sel,
        dst_sel,
        size,
        offset,
        perms
    );

    let tvpe = get_kobj!(vpe, vpe_sel, VPE).upgrade().unwrap();
    if !tvpe.obj_caps().borrow().unused(dst_sel) {
        sysc_err!(Code::InvArgs, "Selector {} already in use", dst_sel);
    }

    let cap = {
        let vpe_caps = vpe.obj_caps().borrow();
        let mgate = get_kobj_ref!(vpe_caps, src_sel, MGate);
        if offset.checked_add(size).is_none() || offset + size > mgate.size() || size == 0 {
            sysc_err!(Code::InvArgs, "Size or offset invalid");
        }

        let addr = mgate.addr().raw() + offset as u64;
        let new_mem = mem::Allocation::new(GlobAddr::new(addr), size);
        let mgate_obj = MGateObject::new(new_mem, perms & mgate.perms(), true);
        Capability::new(dst_sel, KObject::MGate(mgate_obj))
    };

    try_kmem_quota!(tvpe.obj_caps().borrow_mut().insert_as_child(cap, src_sel));

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn derive_srv(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::DeriveSrv = get_request(msg)?;
    let dst_crd = CapRngDesc::new(CapType::OBJECT, req.dst_sel, 2);
    let srv_sel = req.srv_sel as CapSel;
    let sessions = req.sessions as u32;
    let event = req.event;

    sysc_log!(
        vpe,
        "derive_srv(dst={}, srv={}, sessions={}, event={})",
        dst_crd,
        srv_sel,
        sessions,
        event
    );

    if !vpe.obj_caps().borrow().range_unused(&dst_crd) {
        sysc_err!(Code::InvArgs, "Selectors {} already in use", dst_crd);
    }
    if sessions == 0 {
        sysc_err!(Code::InvArgs, "Invalid session count");
    }

    let srvcap = get_kobj!(vpe, srv_sel, Serv);

    let smsg = kif::service::DeriveCreator {
        opcode: kif::service::Operation::DERIVE_CRT.val as u64,
        sessions: sessions as u64,
    };

    // everything worked, send the reply
    reply_success(msg);

    let label = srvcap.creator() as tcu::Label;
    klog!(
        SERV,
        "Sending DERIVE_CRT(sessions={}) to service {} with creator {}",
        sessions,
        srvcap.service().name(),
        label,
    );
    let res = Service::send_receive(srvcap.service(), label, util::object_to_bytes(&smsg));

    let res = match res {
        Err(e) => {
            sysc_log!(
                vpe,
                "Service {} unreachable: {:?}",
                srvcap.service().name(),
                e.code()
            );
            Err(e)
        },

        Ok(rmsg) => {
            let reply: &kif::service::DeriveCreatorReply = get_request(rmsg)?;
            let res = Result::from(Code::from(reply.res as u32));
            let creator = reply.creator as usize;
            let sgate_sel = reply.sgate_sel as CapSel;

            sysc_log!(
                vpe,
                "derive_srv continue with res={:?}, creator={}",
                res,
                creator
            );

            if res.is_ok() {
                // obtain SendGate from server (do that first because it can fail)
                let serv_vpe = srvcap.service().vpe();
                let mut serv_caps = serv_vpe.obj_caps().borrow_mut();
                let src_cap = serv_caps.get_mut(sgate_sel);
                match src_cap {
                    None => sysc_log!(vpe, "Service gave invalid SendGate cap {}", sgate_sel),
                    Some(c) => try_kmem_quota!(vpe.obj_caps().borrow_mut().obtain(
                        dst_crd.start() + 1,
                        c,
                        true
                    )),
                }

                // derive new service object
                let cap = Capability::new(
                    dst_crd.start() + 0,
                    KObject::Serv(ServObject::new(srvcap.service().clone(), false, creator)),
                );
                try_kmem_quota!(vpe.obj_caps().borrow_mut().insert_as_child(cap, srv_sel));
                Ok(())
            }
            else {
                res
            }
        },
    };

    vpe.upcall_derive_srv(event, res);
    Ok(())
}
