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
use base::kif::{self, CapRngDesc, CapSel};
use base::mem::GlobAddr;
use base::rc::{Rc, Weak};
use base::tcu;
use core::intrinsics;

use cap::{Capability, KObject, SelRange};
use cap::{MGateObject, MapObject, RGateObject, SGateObject, SemObject, ServObject, SessObject};
use com::Service;
use mem;
use pes::{pemng, vpemng};
use pes::{VPEFlags, VPE};
use platform;
use syscalls::{get_request, reply_success, send_reply, SyscError};

#[inline(never)]
pub fn create_mgate(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::CreateMGate = get_request(msg)?;
    let dst_sel = req.dst_sel as CapSel;
    let vpe_sel = req.vpe_sel as CapSel;
    let addr = req.addr as goff;
    let size = req.size as goff;
    let perms = kif::Perm::from_bits_truncate(req.perms as u32);

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

    let tgt_vpe: Weak<VPE> = get_kobj!(vpe, vpe_sel, VPE);
    let tgt_vpe = tgt_vpe.upgrade().unwrap();

    let glob = if platform::pe_desc(tgt_vpe.pe_id()).has_virtmem() {
        let sel = (addr / cfg::PAGE_SIZE as goff) as CapSel;
        let mapobj = get_mobj!(tgt_vpe, sel, Map);
        // TODO think about the flags in MapObject again
        let map_perms = kif::Perm::from_bits_truncate(mapobj.flags().bits() as u32);
        if !(perms & !kif::Perm::RWX).is_empty() || !(perms & !map_perms).is_empty() {
            sysc_err!(Code::NoPerm, "Invalid permissions");
        }

        let pages = size / cfg::PAGE_SIZE as goff;
        // TODO validate whether the MapObject starts at sel
        // let off = (addr / cfg::PAGE_SIZE as goff) as CapSel - sel;
        if pages == 0
        /*|| off + pages > mapobj.len()*/
        {
            sysc_err!(Code::InvArgs, "Invalid length");
        }

        #[cfg(target_os = "none")]
        {
            let off = paging::glob_to_phys(mapobj.global().raw());
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

    vpe.obj_caps().borrow_mut().insert_as_child(cap, vpe_sel);

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn create_rgate(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::CreateRGate = get_request(msg)?;
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

    if !vpe.obj_caps().borrow().unused(dst_sel) {
        sysc_err!(Code::InvArgs, "Selector {} already in use", dst_sel);
    }
    if order <= 0
        || msg_order <= 0
        || msg_order.checked_add(order).is_none()
        || msg_order > order
        || order - msg_order >= 32
        || (1 << (order - msg_order)) > cfg::MAX_RB_SIZE
    {
        sysc_err!(Code::InvArgs, "Invalid size");
    }

    vpe.obj_caps().borrow_mut().insert(Capability::new(
        dst_sel,
        KObject::RGate(RGateObject::new(order, msg_order)),
    ));

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn create_sgate(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::CreateSGate = get_request(msg)?;
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

    if !vpe.obj_caps().borrow().unused(dst_sel) {
        sysc_err!(Code::InvArgs, "Selector {} already in use", dst_sel);
    }

    let rgate: Rc<RGateObject> = get_kobj!(vpe, rgate_sel, RGate);
    let cap = Capability::new(
        dst_sel,
        KObject::SGate(SGateObject::new(&rgate, label, credits)),
    );

    vpe.obj_caps().borrow_mut().insert_as_child(cap, rgate_sel);

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn create_srv(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::CreateSrv = get_request(msg)?;
    let dst_sel = req.dst_sel as CapSel;
    let rgate_sel = req.rgate_sel as CapSel;
    let creator = req.creator as usize;
    let name: &str = unsafe { intrinsics::transmute(&req.name[0..req.namelen as usize]) };

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
    if name.len() == 0 {
        sysc_err!(Code::InvArgs, "Invalid server name");
    }

    let rgate: Rc<RGateObject> = get_kobj!(vpe, rgate_sel, RGate);
    if !rgate.activated() {
        sysc_err!(Code::InvArgs, "RGate is not activated");
    }

    let serv = Service::new(vpe, name.to_string(), rgate);
    let cap = Capability::new(dst_sel, KObject::Serv(ServObject::new(serv, true, creator)));
    vpe.obj_caps().borrow_mut().insert(cap);

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn create_sess(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::CreateSess = get_request(msg)?;
    let dst_sel = req.dst_sel as CapSel;
    let srv_sel = req.srv_sel as CapSel;
    let creator = req.creator as usize;
    let ident = req.ident;

    sysc_log!(
        vpe,
        "create_sess(dst={}, srv={}, creator={}, ident={:#x})",
        dst_sel,
        srv_sel,
        creator,
        ident
    );

    if !vpe.obj_caps().borrow().unused(dst_sel) {
        sysc_err!(Code::InvArgs, "Selector {} already in use", dst_sel);
    }

    let serv: Rc<ServObject> = get_kobj!(vpe, srv_sel, Serv);
    let cap = Capability::new(
        dst_sel,
        KObject::Sess(SessObject::new(&serv, creator, ident)),
    );

    vpe.obj_caps().borrow_mut().insert_as_child(cap, srv_sel);

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn create_vpe(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::CreateVPE = get_request(msg)?;
    let dst_crd = CapRngDesc::new_from(req.dst_crd);
    let pg_sg_sel = req.pg_sg_sel as CapSel;
    let pg_rg_sel = req.pg_rg_sel as CapSel;
    let pe_sel = req.pe_sel as CapSel;
    let kmem_sel = req.kmem_sel as CapSel;
    let name: &str = unsafe { intrinsics::transmute(&req.name[0..req.namelen as usize]) };

    sysc_log!(
        vpe,
        "create_vpe(dst={}, pg_sg={}, pg_rg={}, name={}, pe={}, kmem={})",
        dst_crd,
        pg_sg_sel,
        pg_rg_sel,
        name,
        pe_sel,
        kmem_sel
    );

    let cap_count = kif::FIRST_FREE_SEL;
    if dst_crd.count() != cap_count || !vpe.obj_caps().borrow().range_unused(&dst_crd) {
        sysc_err!(Code::InvArgs, "Selectors {} already in use", dst_crd);
    }
    if name.len() == 0 {
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
        let sgate = if pg_sg_sel != kif::INVALID_SEL {
            Some(get_kobj!(vpe, pg_sg_sel, SGate))
        }
        else {
            None
        };

        let rgate = if pg_rg_sel != kif::INVALID_SEL {
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

    let kmem = get_kobj!(vpe, kmem_sel, KMEM);
    // TODO kmem quota stuff

    // find contiguous space for standard EPs
    let pemux = pemng::get().pemux(pe.pe());
    let eps = pemux
        .find_eps(tcu::STD_EPS_COUNT as u32)
        .map_err(|e| SyscError::new(e.code(), "No free range for standard EPs".to_string()))?;
    if pemux.has_vpes() && !pe_desc.has_virtmem() {
        sysc_err!(Code::NotSup, "Virtual memory is required for PE sharing");
    }

    // create VPE
    let nvpe: Rc<VPE> = vpemng::get()
        .create(name, pe, eps, kmem, VPEFlags::empty())
        .map_err(|e| SyscError::new(e.code(), "Unable to create VPE".to_string()))?;

    // inherit VPE and EP caps to the parent
    for sel in kif::SEL_VPE..cap_count {
        let mut obj_caps = nvpe.obj_caps().borrow_mut();
        let cap: Option<&mut Capability> = obj_caps.get_mut(sel);
        cap.map(|c| {
            vpe.obj_caps()
                .borrow_mut()
                .obtain(dst_crd.start() + sel, c, false)
        });
    }

    // activate pager EPs
    #[cfg(target_os = "none")]
    {
        if let Some(sg) = _sgate {
            // TODO remember the EP to invalidate it on gate destruction
            pemux
                .config_snd_ep(eps + tcu::PG_SEP_OFF, nvpe.id(), &sg)
                .unwrap();
        }
        if let Some(mut rg) = _rgate {
            let rbuf = nvpe.rbuf_addr()
                + cfg::SYSC_RBUF_SIZE as goff
                + cfg::UPCALL_RBUF_SIZE as goff
                + cfg::DEF_RBUF_SIZE as goff;
            rg.activate(nvpe.pe_id(), eps + tcu::PG_REP_OFF, rbuf);
            pemux
                .config_rcv_ep(eps + tcu::PG_REP_OFF, nvpe.id(), None, &mut rg)
                .unwrap();
        }
    }

    let kreply = kif::syscalls::CreateVPEReply {
        error: 0,
        eps_start: eps as u64,
    };
    send_reply(msg, &kreply);

    Ok(())
}

#[inline(never)]
pub fn create_sem(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::CreateSem = get_request(msg)?;
    let dst_sel = req.dst_sel as CapSel;
    let value = req.value as u32;

    sysc_log!(vpe, "create_sem(dst={}, value={})", dst_sel, value);

    if !vpe.obj_caps().borrow().unused(dst_sel) {
        sysc_err!(Code::InvArgs, "Selector {} already in use", dst_sel);
    }

    let cap = Capability::new(dst_sel, KObject::Sem(SemObject::new(value)));
    vpe.obj_caps().borrow_mut().insert(cap);

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn create_map(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::CreateMap = get_request(msg)?;
    let dst_sel = req.dst_sel as CapSel;
    let mgate_sel = req.mgate_sel as CapSel;
    let vpe_sel = req.vpe_sel as CapSel;
    let first = req.first as CapSel;
    let pages = req.pages as CapSel;
    let perms = kif::Perm::from_bits_truncate(req.perms as u32);

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

    let dst_vpe: Weak<VPE> = get_kobj!(vpe, vpe_sel, VPE);
    let dst_vpe = dst_vpe.upgrade().unwrap();
    let mgate: Rc<MGateObject> = get_kobj!(vpe, mgate_sel, MGate);

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
                if c.len() != pages {
                    sysc_err!(Code::InvArgs, "Map cap exists with different page count");
                }
                (c.get().clone(), true)
            },
            None => (
                KObject::Map(MapObject::new(phys, kif::PageFlags::from(perms))),
                false,
            ),
        }
    };

    // create/update the PTEs
    if let KObject::Map(m) = &map_obj {
        m.remap(
            &dst_vpe,
            virt,
            phys,
            pages as usize,
            kif::PageFlags::from(perms),
        )
        .map_err(|e| SyscError::new(e.code(), "Unable to map memory".to_string()))?;
    }

    // create map cap, if not yet existing
    if !exists {
        let cap = Capability::new_range(SelRange::new_range(dst_sel, pages), map_obj.clone());
        if vpe_sel == kif::SEL_VPE {
            vpe.map_caps().borrow_mut().insert_as_child(cap, mgate_sel);
        }
        else {
            dst_vpe.map_caps().borrow_mut().insert_as_child_from(
                cap,
                vpe.obj_caps().borrow_mut(),
                mgate_sel,
            );
        }
    }

    reply_success(msg);
    Ok(())
}
