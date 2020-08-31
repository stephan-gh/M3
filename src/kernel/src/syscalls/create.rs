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

use base::cfg;
use base::col::ToString;
use base::errors::Code;
use base::goff;
use base::kif::{syscalls, CapRngDesc, CapSel, CapType, PageFlags, Perm, INVALID_SEL};
use base::mem::GlobAddr;
use base::rc::Rc;
use base::tcu;

use crate::cap::{Capability, KObject, SelRange};
use crate::cap::{
    MGateObject, MapObject, RGateObject, SGateObject, SemObject, ServObject, SessObject,
};
use crate::com::Service;
use crate::mem;
use crate::pes::{PEMng, VPEFlags, VPEMng, VPE};
use crate::platform;
use crate::syscalls::{get_request, reply_success, send_reply, SyscError};

#[inline(never)]
pub fn create_mgate(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &syscalls::CreateMGate = get_request(msg)?;
    let dst_sel = req.dst_sel as CapSel;
    let vpe_sel = req.vpe_sel as CapSel;
    let addr = req.addr as goff;
    let size = req.size as goff;
    let perms = Perm::from_bits_truncate(req.perms as u32);

    sysc_log!(
        vpe,
        "create_mgate(dst={}, vpe={}, addr={:#x}, size={:#x}, perms={:?})",
        dst_sel,
        vpe_sel,
        addr,
        size,
        perms,
    );

    if !vpe.obj_caps().borrow().unused(dst_sel) {
        sysc_err!(Code::InvArgs, "Selector {} already in use", dst_sel);
    }
    if (addr & cfg::PAGE_MASK as goff) != 0 || (size & cfg::PAGE_MASK as goff) != 0 {
        sysc_err!(
            Code::InvArgs,
            "Virt address and size need to be page-aligned"
        );
    }

    let tgt_vpe = get_kobj!(vpe, vpe_sel, VPE).upgrade().unwrap();

    let sel = (addr / cfg::PAGE_SIZE as goff) as CapSel;
    let glob = if platform::pe_desc(tgt_vpe.pe_id()).has_virtmem() {
        let map_caps = tgt_vpe.map_caps().borrow();
        let map_cap = map_caps
            .get(sel)
            .ok_or_else(|| SyscError::new(Code::InvArgs, "Invalid capability".to_string()))?;
        let map_obj = as_obj!(map_cap.get(), Map);

        // TODO think about the flags in MapObject again
        let map_perms = Perm::from_bits_truncate(map_obj.flags().bits() as u32);
        if !(perms & !Perm::RWX).is_empty() || !(perms & !map_perms).is_empty() {
            sysc_err!(Code::NoPerm, "Invalid permissions");
        }

        let pages = (size / cfg::PAGE_SIZE as goff) as CapSel;
        let off = (addr / cfg::PAGE_SIZE as goff) as CapSel - map_cap.sel();
        if pages == 0 || off + pages > map_cap.len() {
            sysc_err!(Code::InvArgs, "Invalid length");
        }

        #[cfg(target_os = "none")]
        {
            let off = paging::glob_to_phys(map_obj.global().raw());
            GlobAddr::new_with(tgt_vpe.pe_id(), off)
        }
        #[cfg(target_os = "linux")]
        GlobAddr::new(0)
    }
    else {
        if size == 0 || addr + size >= cfg::MEM_CAP_END as goff {
            sysc_err!(Code::InvArgs, "Region empty or out of bounds");
        }

        GlobAddr::new_with(tgt_vpe.pe_id(), addr)
    };

    let mem = mem::Allocation::new(glob, size);
    let cap = Capability::new(dst_sel, KObject::MGate(MGateObject::new(mem, perms, true)));

    if platform::pe_desc(tgt_vpe.pe_id()).has_virtmem() {
        let map_caps = tgt_vpe.map_caps().borrow_mut();
        try_kmem_quota!(vpe
            .obj_caps()
            .borrow_mut()
            .insert_as_child_from(cap, map_caps, sel));
    }
    else {
        try_kmem_quota!(vpe.obj_caps().borrow_mut().insert_as_child(cap, vpe_sel));
    }

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn create_rgate(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &syscalls::CreateRGate = get_request(msg)?;
    let dst_sel = req.dst_sel as CapSel;
    let order = req.order as u32;
    let msg_order = req.msgorder as u32;

    sysc_log!(
        vpe,
        "create_rgate(dst={}, size={:#x}, msg_size={:#x})",
        dst_sel,
        1u32.checked_shl(order).unwrap_or(0),
        1u32.checked_shl(msg_order).unwrap_or(0)
    );

    let mut vpe_caps = vpe.obj_caps().borrow_mut();

    if !vpe_caps.unused(dst_sel) {
        sysc_err!(Code::InvArgs, "Selector {} already in use", dst_sel);
    }
    if msg_order.checked_add(order).is_none()
        || msg_order > order
        || order - msg_order >= 32
        || (1 << (order - msg_order)) > cfg::MAX_RB_SIZE
    {
        sysc_err!(Code::InvArgs, "Invalid size");
    }

    try_kmem_quota!(vpe_caps.insert(Capability::new(
        dst_sel,
        KObject::RGate(RGateObject::new(order, msg_order)),
    )));

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn create_sgate(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &syscalls::CreateSGate = get_request(msg)?;
    let dst_sel = req.dst_sel as CapSel;
    let rgate_sel = req.rgate_sel as CapSel;
    let label = req.label as tcu::Label;
    let credits = req.credits as u32;

    sysc_log!(
        vpe,
        "create_sgate(dst={}, rgate={}, label={:#x}, credits={})",
        dst_sel,
        rgate_sel,
        label,
        credits
    );

    let mut vpe_caps = vpe.obj_caps().borrow_mut();

    if !vpe_caps.unused(dst_sel) {
        sysc_err!(Code::InvArgs, "Selector {} already in use", dst_sel);
    }

    let cap = {
        let rgate = get_kobj_ref!(vpe_caps, rgate_sel, RGate);
        Capability::new(
            dst_sel,
            KObject::SGate(SGateObject::new(&rgate, label, credits)),
        )
    };

    try_kmem_quota!(vpe_caps.insert_as_child(cap, rgate_sel));

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn create_srv(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &syscalls::CreateSrv = get_request(msg)?;
    let dst_sel = req.dst_sel as CapSel;
    let rgate_sel = req.rgate_sel as CapSel;
    let creator = req.creator as usize;
    let name = core::str::from_utf8(&req.name[0..req.namelen as usize])
        .map_err(|_| SyscError::new(Code::InvArgs, "Invalid name".to_string()))?;

    sysc_log!(
        vpe,
        "create_srv(dst={}, rgate={}, creator={}, name={})",
        dst_sel,
        rgate_sel,
        creator,
        name
    );

    if !vpe.obj_caps().borrow().unused(dst_sel) {
        sysc_err!(Code::InvArgs, "Selector {} already in use", dst_sel);
    }
    if name.is_empty() {
        sysc_err!(Code::InvArgs, "Invalid server name");
    }

    let mut vpe_caps = vpe.obj_caps().borrow_mut();

    let cap = {
        let rgate = get_kobj_ref!(vpe_caps, rgate_sel, RGate);
        if !rgate.activated() {
            sysc_err!(Code::InvArgs, "RGate is not activated");
        }

        let serv = Service::new(vpe, name.to_string(), rgate.clone());
        Capability::new(dst_sel, KObject::Serv(ServObject::new(serv, true, creator)))
    };

    try_kmem_quota!(vpe_caps.insert(cap));

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn create_sess(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &syscalls::CreateSess = get_request(msg)?;
    let dst_sel = req.dst_sel as CapSel;
    let srv_sel = req.srv_sel as CapSel;
    let creator = req.creator as usize;
    let ident = req.ident;
    let auto_close = req.auto_close != 0;

    sysc_log!(
        vpe,
        "create_sess(dst={}, srv={}, creator={}, ident={:#x}, auto_close={})",
        dst_sel,
        srv_sel,
        creator,
        ident,
        auto_close
    );

    let mut obj_caps = vpe.obj_caps().borrow_mut();
    if !obj_caps.unused(dst_sel) {
        sysc_err!(Code::InvArgs, "Selector {} already in use", dst_sel);
    }

    let serv_cap = get_cap!(obj_caps, srv_sel);
    if serv_cap.has_parent() {
        sysc_err!(Code::InvArgs, "Only the service owner can create sessions");
    }

    let serv = as_obj!(serv_cap.get(), Serv);
    // TODO implement auto_close
    let cap = Capability::new(
        dst_sel,
        KObject::Sess(SessObject::new(&serv, creator, ident)),
    );

    try_kmem_quota!(obj_caps.insert_as_child(cap, srv_sel));

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn create_vpe_async(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &syscalls::CreateVPE = get_request(msg)?;
    let dst_sel = req.dst_sel as CapSel;
    let pg_sg_sel = req.pg_sg_sel as CapSel;
    let pg_rg_sel = req.pg_rg_sel as CapSel;
    let pe_sel = req.pe_sel as CapSel;
    let kmem_sel = req.kmem_sel as CapSel;
    let name = core::str::from_utf8(&req.name[0..req.namelen as usize])
        .map_err(|_| SyscError::new(Code::InvArgs, "Invalid name".to_string()))?;

    sysc_log!(
        vpe,
        "create_vpe(dst={}, pg_sg={}, pg_rg={}, name={}, pe={}, kmem={})",
        dst_sel,
        pg_sg_sel,
        pg_rg_sel,
        name,
        pe_sel,
        kmem_sel
    );

    if !vpe.obj_caps().borrow().unused(dst_sel) {
        sysc_err!(Code::InvArgs, "Selector {} already in use", dst_sel);
    }
    if name.is_empty() {
        sysc_err!(Code::InvArgs, "Invalid name");
    }

    let pe = get_kobj!(vpe, pe_sel, PE);
    if !pe.has_quota(tcu::STD_EPS_COUNT as u32) {
        sysc_err!(
            Code::InvArgs,
            "PE cap has insufficient EPs (have {}, need {})",
            pe.eps(),
            tcu::STD_EPS_COUNT
        );
    }

    // on VM PEs, we need sgate/rgate caps
    let pe_desc = platform::pe_desc(pe.pe());
    let (_sgate, _rgate) = if pe_desc.has_virtmem() {
        let sgate = if pg_sg_sel != INVALID_SEL {
            Some(get_kobj!(vpe, pg_sg_sel, SGate))
        }
        else {
            None
        };

        let rgate = if pg_rg_sel != INVALID_SEL {
            let rgate = get_kobj!(vpe, pg_rg_sel, RGate);
            if rgate.activated() {
                sysc_err!(Code::InvArgs, "Pager rgate already activated");
            }
            Some(rgate)
        }
        else {
            None
        };
        (sgate, rgate)
    }
    else {
        (None, None)
    };

    let kmem = get_kobj!(vpe, kmem_sel, KMem);
    // TODO kmem quota stuff

    // find contiguous space for standard EPs
    let pemux = PEMng::get().pemux(pe.pe());
    let eps = match pemux.find_eps(tcu::STD_EPS_COUNT as u32) {
        Ok(eps) => eps,
        Err(e) => sysc_err!(e.code(), "No free range for standard EPs"),
    };
    if pemux.has_vpes() && !pe_desc.has_virtmem() {
        sysc_err!(Code::NotSup, "Virtual memory is required for PE sharing");
    }

    // create VPE
    let nvpe = match VPEMng::get().create_vpe_async(name, pe, eps, kmem, VPEFlags::empty()) {
        Ok(nvpe) => nvpe,
        Err(e) => sysc_err!(e.code(), "Unable to create VPE"),
    };

    // give VPE cap to the parent
    let cap = Capability::new(dst_sel, KObject::VPE(Rc::downgrade(&nvpe)));
    try_kmem_quota!(vpe.obj_caps().borrow_mut().insert(cap));

    // activate pager EPs
    #[cfg(target_os = "none")]
    {
        use crate::cap::EPObject;

        if let Some(sg) = _sgate {
            pemux
                .config_snd_ep(eps + tcu::PG_SEP_OFF, nvpe.id(), &sg)
                .unwrap();

            // remember the activation
            let sep = EPObject::new(true, &nvpe, eps + tcu::PG_SEP_OFF, 0, pemux.pe());
            EPObject::configure(&sep, &KObject::SGate(sg));
            nvpe.add_ep(sep);
        }
        if let Some(rg) = _rgate {
            let rbuf = nvpe.rbuf_addr()
                + cfg::SYSC_RBUF_SIZE as goff
                + cfg::UPCALL_RBUF_SIZE as goff
                + cfg::DEF_RBUF_SIZE as goff;
            rg.activate(nvpe.pe_id(), eps + tcu::PG_REP_OFF, rbuf);

            pemux
                .config_rcv_ep(eps + tcu::PG_REP_OFF, nvpe.id(), None, &rg)
                .unwrap();

            // remember the activation
            let rep = EPObject::new(true, &nvpe, eps + tcu::PG_REP_OFF, 0, pemux.pe());
            EPObject::configure(&rep, &KObject::RGate(rg));
            nvpe.add_ep(rep);
        }
    }

    let kreply = syscalls::CreateVPEReply {
        error: 0,
        eps_start: eps as u64,
    };
    send_reply(msg, &kreply);

    Ok(())
}

#[inline(never)]
pub fn create_sem(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &syscalls::CreateSem = get_request(msg)?;
    let dst_sel = req.dst_sel as CapSel;
    let value = req.value as u32;

    sysc_log!(vpe, "create_sem(dst={}, value={})", dst_sel, value);

    if !vpe.obj_caps().borrow().unused(dst_sel) {
        sysc_err!(Code::InvArgs, "Selector {} already in use", dst_sel);
    }

    let cap = Capability::new(dst_sel, KObject::Sem(SemObject::new(value)));
    try_kmem_quota!(vpe.obj_caps().borrow_mut().insert(cap));

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn create_map_async(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &syscalls::CreateMap = get_request(msg)?;
    let dst_sel = req.dst_sel as CapSel;
    let mgate_sel = req.mgate_sel as CapSel;
    let vpe_sel = req.vpe_sel as CapSel;
    let first = req.first as CapSel;
    let pages = req.pages as CapSel;
    let perms = Perm::from_bits_truncate(req.perms as u32);

    sysc_log!(
        vpe,
        "create_map(dst={}, vpe={}, mgate={}, first={}, pages={}, perms={:?})",
        dst_sel,
        vpe_sel,
        mgate_sel,
        first,
        pages,
        perms
    );

    let dst_vpe = get_kobj!(vpe, vpe_sel, VPE).upgrade().unwrap();
    if !platform::pe_desc(dst_vpe.pe_id()).has_virtmem() {
        sysc_err!(Code::InvArgs, "PE has no virtual-memory support");
    }

    let mgate = get_kobj!(vpe, mgate_sel, MGate);
    if (mgate.addr().raw() & cfg::PAGE_MASK as goff) != 0
        || (mgate.size() & cfg::PAGE_MASK as goff) != 0
    {
        sysc_err!(
            Code::InvArgs,
            "Memory capability is not page aligned (addr={:?}, size={:#x})",
            mgate.addr(),
            mgate.size()
        );
    }
    if (perms.bits() & !mgate.perms().bits()) != 0 {
        sysc_err!(Code::InvArgs, "Invalid permissions");
    }

    let total_pages = (mgate.size() >> cfg::PAGE_BITS) as CapSel;
    if first.checked_add(pages).is_none()
        || pages == 0
        || first >= total_pages
        || first + pages > total_pages
    {
        sysc_err!(Code::InvArgs, "Region of memory cap is invalid");
    }

    let virt = (dst_sel as goff) << cfg::PAGE_BITS;
    let base = mgate.addr().raw();
    let phys = GlobAddr::new(base + (cfg::PAGE_SIZE * first as usize) as u64);

    // retrieve/create map object
    let (map_obj, exists) = {
        let map_caps = dst_vpe.map_caps().borrow();
        let map_cap: Option<&Capability> = map_caps.get(dst_sel);
        match map_cap {
            Some(c) => {
                // TODO check for kernel-created caps
                // TODO we have to update MemGates that are childs of this cap
                if c.len() != pages {
                    sysc_err!(Code::InvArgs, "Map cap exists with different page count");
                }

                (c.get().clone(), true)
            },
            None => {
                let range = CapRngDesc::new(CapType::MAPPING, dst_sel, pages);
                if !map_caps.range_unused(&range) {
                    sysc_err!(Code::InvArgs, "Capability range {} already in use", range);
                }

                let kobj = KObject::Map(MapObject::new(phys, PageFlags::from(perms)));
                (kobj, false)
            },
        }
    };

    // create/update the PTEs
    if let KObject::Map(m) = &map_obj {
        if let Err(e) = m.map_async(&dst_vpe, virt, phys, pages as usize, PageFlags::from(perms)) {
            sysc_err!(e.code(), "Unable to map memory");
        }
    }

    // create map cap, if not yet existing
    if !exists {
        let cap = Capability::new_range(SelRange::new_range(dst_sel, pages), map_obj);
        try_kmem_quota!(dst_vpe.map_caps().borrow_mut().insert_as_child_from(
            cap,
            vpe.obj_caps().borrow_mut(),
            mgate_sel,
        ));
    }

    reply_success(msg);
    Ok(())
}
