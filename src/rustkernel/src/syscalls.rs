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
use base::col::{String, ToString};
use base::errors::{Code, Error};
use base::goff;
use base::kif::{self, CapRngDesc, CapSel, CapType};
use base::mem::GlobAddr;
use base::rc::Rc;
use base::tcu;
use base::util;
use core::intrinsics;
use thread;

use arch::loader::Loader;
use cap::{Capability, KObject, SelRange};
use cap::{
    EPObject, GateObject, KMemObject, MGateObject, MapObject, PEObject, RGateObject, SGateObject,
    SemObject, ServObject, SessObject,
};
use com::Service;
use ktcu;
use mem;
use pes::{pemng, vpemng};
use pes::{VPEFlags, VPE};
use platform;

// TODO split the syscalls into multiple files

macro_rules! sysc_log {
    ($vpe:expr, $fmt:tt, $($args:tt)*) => (
        klog!(
            SYSC,
            concat!("{}:{}@{}: syscall::", $fmt),
            $vpe.id(), $vpe.name(), $vpe.pe_id(), $($args)*
        )
    )
}

macro_rules! sysc_err {
    ($e:expr, $fmt:tt) => ({
        return Err(SyscError::new($e, $fmt.to_string()));
    });
    ($e:expr, $fmt:tt, $($args:tt)*) => ({
        return Err(SyscError::new($e, format!($fmt, $($args)*)));
    });
}

macro_rules! get_mobj {
    ($vpe:expr, $sel:expr, $ty:ident) => {
        get_obj!($vpe, $sel, $ty, map_caps)
    };
}
macro_rules! get_kobj {
    ($vpe:expr, $sel:expr, $ty:ident) => {
        get_obj!($vpe, $sel, $ty, obj_caps)
    };
}
macro_rules! get_obj {
    ($vpe:expr, $sel:expr, $ty:ident, $table:tt) => {{
        let kobj = match $vpe.$table().borrow().get($sel) {
            Some(c) => c.get().clone(),
            None => sysc_err!(Code::InvArgs, "Invalid capability"),
        };
        // TODO wasn't there a crate that allows to use just "if" for that?
        match kobj {
            KObject::$ty(k) => k,
            _ => sysc_err!(Code::InvArgs, "Expected {:?} cap", stringify!($ty)),
        }
    }};
}

struct SyscError {
    pub code: Code,
    pub msg: String,
}

impl SyscError {
    pub fn new(code: Code, msg: String) -> Self {
        SyscError { code, msg }
    }
}

impl From<Error> for SyscError {
    fn from(e: Error) -> Self {
        SyscError::new(e.code(), String::default())
    }
}

fn get_message<R: 'static>(msg: &'static tcu::Message) -> &'static R {
    // TODO use Message::get_data instead
    let data: &[R] = unsafe { intrinsics::transmute(&msg.data) };
    &data[0]
}

fn send_reply<T>(msg: &'static tcu::Message, rep: &T) {
    ktcu::reply(ktcu::KSYS_EP, rep, msg).ok();
}

fn reply_result(msg: &'static tcu::Message, code: u64) {
    let rep = kif::DefaultReply { error: code };
    send_reply(msg, &rep);
}

fn reply_success(msg: &'static tcu::Message) {
    reply_result(msg, 0);
}

pub fn handle(msg: &'static tcu::Message) {
    let vpe: Rc<VPE> = vpemng::get().vpe(msg.header.label as usize).unwrap();
    let opcode: &u64 = get_message(msg);

    let res = match kif::syscalls::Operation::from(*opcode) {
        kif::syscalls::Operation::CREATE_MGATE => create_mgate(&vpe, msg),
        kif::syscalls::Operation::CREATE_RGATE => create_rgate(&vpe, msg),
        kif::syscalls::Operation::CREATE_SGATE => create_sgate(&vpe, msg),
        kif::syscalls::Operation::CREATE_SRV => create_srv(&vpe, msg),
        kif::syscalls::Operation::CREATE_SESS => create_sess(&vpe, msg),
        kif::syscalls::Operation::CREATE_VPE => create_vpe(&vpe, msg),
        kif::syscalls::Operation::CREATE_SEM => create_sem(&vpe, msg),
        kif::syscalls::Operation::CREATE_MAP => create_map(&vpe, msg),

        kif::syscalls::Operation::ALLOC_EP => alloc_ep(&vpe, msg),
        kif::syscalls::Operation::ACTIVATE => activate(&vpe, msg),
        kif::syscalls::Operation::KMEM_QUOTA => kmem_quota(&vpe, msg),
        kif::syscalls::Operation::PE_QUOTA => pe_quota(&vpe, msg),
        kif::syscalls::Operation::DERIVE_PE => derive_pe(&vpe, msg),
        kif::syscalls::Operation::DERIVE_MEM => derive_mem(&vpe, msg),
        kif::syscalls::Operation::DERIVE_KMEM => derive_kmem(&vpe, msg),
        kif::syscalls::Operation::DERIVE_SRV => derive_srv(&vpe, msg),
        kif::syscalls::Operation::GET_SESS => get_sess(&vpe, msg),
        kif::syscalls::Operation::SEM_CTRL => sem_ctrl(&vpe, msg),
        kif::syscalls::Operation::VPE_CTRL => vpe_ctrl(&vpe, msg),
        kif::syscalls::Operation::VPE_WAIT => vpe_wait(&vpe, msg),

        kif::syscalls::Operation::EXCHANGE => exchange(&vpe, msg),
        kif::syscalls::Operation::DELEGATE => exchange_over_sess(&vpe, msg, false),
        kif::syscalls::Operation::OBTAIN => exchange_over_sess(&vpe, msg, true),
        kif::syscalls::Operation::REVOKE => revoke(&vpe, msg),

        kif::syscalls::Operation::NOOP => noop(&vpe, msg),
        _ => panic!("Unexpected operation: {}", opcode),
    };

    if let Err(e) = res {
        if e.msg.len() == 0 {
            klog!(
                ERR,
                "\x1B[37;41m{}:{}@{}: {:?} failed: {:?}\x1B[0m",
                vpe.id(),
                vpe.name(),
                vpe.pe_id(),
                kif::syscalls::Operation::from(*opcode),
                e.code
            );
        }
        else {
            klog!(
                ERR,
                "\x1B[37;41m{}:{}@{}: {:?} failed: {} ({:?})\x1B[0m",
                vpe.id(),
                vpe.name(),
                vpe.pe_id(),
                kif::syscalls::Operation::from(*opcode),
                e.msg,
                e.code
            );
        }

        reply_result(msg, e.code as u64);
    }
}

#[inline(never)]
fn create_mgate(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::CreateMGate = get_message(msg);
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

    let tgt_vpe: Rc<VPE> = get_kobj!(vpe, vpe_sel, VPE);

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

    {
        let mem = mem::Allocation::new(glob, size);
        let cap = Capability::new(dst_sel, KObject::MGate(MGateObject::new(mem, perms, true)));

        vpe.obj_caps().borrow_mut().insert_as_child(cap, vpe_sel);
    }

    reply_success(msg);
    Ok(())
}

#[inline(never)]
fn create_rgate(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::CreateRGate = get_message(msg);
    let dst_sel = req.dst_sel as CapSel;
    let order = req.order as u32;
    let msg_order = req.msgorder as u32;

    sysc_log!(
        vpe,
        "create_rgate(dst={}, size={:#x}, msg_size={:#x})",
        dst_sel,
        1 << order,
        1 << msg_order
    );

    if !vpe.obj_caps().borrow().unused(dst_sel) {
        sysc_err!(Code::InvArgs, "Selector {} already in use", dst_sel);
    }
    if order <= 0
        || msg_order <= 0
        || msg_order + order < msg_order
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
fn create_sgate(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::CreateSGate = get_message(msg);
    let dst_sel = req.dst_sel as CapSel;
    let rgate_sel = req.rgate_sel as CapSel;
    let label = req.label as tcu::Label;
    let credits = req.credits as u32;

    sysc_log!(
        vpe,
        "create_sgate(dst={}, rgate={}, label={:#x}, credits={:#x})",
        dst_sel,
        rgate_sel,
        label,
        credits
    );

    if !vpe.obj_caps().borrow().unused(dst_sel) {
        sysc_err!(Code::InvArgs, "Selector {} already in use", dst_sel);
    }

    {
        let rgate: Rc<RGateObject> = get_kobj!(vpe, rgate_sel, RGate);
        let cap = Capability::new(
            dst_sel,
            KObject::SGate(SGateObject::new(&rgate, label, credits)),
        );

        vpe.obj_caps().borrow_mut().insert_as_child(cap, rgate_sel);
    }

    reply_success(msg);
    Ok(())
}

#[inline(never)]
fn create_srv(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::CreateSrv = get_message(msg);
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
fn create_sess(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::CreateSess = get_message(msg);
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

    let cap = {
        let serv: Rc<ServObject> = get_kobj!(vpe, srv_sel, Serv);
        Capability::new(
            dst_sel,
            KObject::Sess(SessObject::new(&serv, creator, ident)),
        )
    };

    vpe.obj_caps().borrow_mut().insert_as_child(cap, srv_sel);

    reply_success(msg);
    Ok(())
}

#[inline(never)]
fn create_vpe(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::CreateVPE = get_message(msg);
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
    let eps = pemux.find_eps(tcu::STD_EPS_COUNT as u32)?;
    if pemux.has_vpes() && !pe_desc.has_virtmem() {
        sysc_err!(Code::NotSup, "Virtual memory is required for PE sharing");
    }

    // create VPE
    let nvpe: Rc<VPE> = vpemng::get().create(name, pe, eps, kmem, VPEFlags::empty())?;

    // inherit VPE and EP caps to the parent
    {
        for sel in kif::SEL_VPE..cap_count {
            let mut obj_caps = nvpe.obj_caps().borrow_mut();
            let cap: Option<&mut Capability> = obj_caps.get_mut(sel);
            cap.map(|c| {
                vpe.obj_caps()
                    .borrow_mut()
                    .obtain(dst_crd.start() + sel, c, false)
            });
        }
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
fn create_sem(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::CreateSem = get_message(msg);
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
fn create_map(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::CreateMap = get_message(msg);
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

    let dst_vpe: Rc<VPE> = get_kobj!(vpe, vpe_sel, VPE);
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
    if first + pages <= first || first >= total_pages || first + pages > total_pages {
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
            pages as usize,
            phys,
            kif::PageFlags::from(perms),
        )?;
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

#[inline(never)]
fn alloc_ep(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::AllocEP = get_message(msg);
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
    let dst_vpe: Rc<VPE> = get_kobj!(vpe, vpe_sel, VPE);
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
        epid = pemux.find_eps(ep_count)?;
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
fn kmem_quota(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::KMemQuota = get_message(msg);
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
fn pe_quota(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::PEQuota = get_message(msg);
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
fn derive_pe(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::DerivePE = get_message(msg);
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
    vpe.obj_caps().borrow_mut().insert_as_child(cap, pe_sel);
    pe.alloc(eps);

    reply_success(msg);
    Ok(())
}

#[inline(never)]
fn derive_kmem(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::DeriveKMem = get_message(msg);
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

    let kmem: Rc<KMemObject> = get_kobj!(vpe, kmem_sel, KMEM);

    if !kmem.has_quota(quota) {
        sysc_err!(Code::NoSpace, "Insufficient quota");
    }

    let cap = Capability::new(dst_sel, KObject::KMEM(KMemObject::new(quota)));
    vpe.obj_caps().borrow_mut().insert_as_child(cap, kmem_sel);
    kmem.alloc(quota);

    reply_success(msg);
    Ok(())
}

#[inline(never)]
fn derive_mem(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::DeriveMem = get_message(msg);
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

    let tvpe = get_kobj!(vpe, vpe_sel, VPE);
    if !tvpe.obj_caps().borrow().unused(dst_sel) {
        sysc_err!(Code::InvArgs, "Selector {} already in use", dst_sel);
    }

    let cap = {
        let mgate: Rc<MGateObject> = get_kobj!(vpe, src_sel, MGate);

        if offset + size < offset || offset + size > mgate.size() || size == 0 {
            sysc_err!(Code::InvArgs, "Size or offset invalid");
        }

        let addr = mgate.addr().raw() + offset as u64;
        let new_mem = mem::Allocation::new(GlobAddr::new(addr), size);
        let mgate_obj = MGateObject::new(new_mem, perms & mgate.perms(), true);
        Capability::new(dst_sel, KObject::MGate(mgate_obj))
    };

    tvpe.obj_caps().borrow_mut().insert_as_child(cap, src_sel);

    reply_success(msg);
    Ok(())
}

#[inline(never)]
fn derive_srv(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::DeriveSrv = get_message(msg);
    let dst_crd = CapRngDesc::new_from(req.dst_crd);
    let srv_sel = req.srv_sel as CapSel;
    let sessions = req.sessions as u32;

    sysc_log!(
        vpe,
        "derive_srv(dst={}, srv={}, sessions={})",
        dst_crd,
        srv_sel,
        sessions
    );

    if dst_crd.count() != 2 || !vpe.obj_caps().borrow().range_unused(&dst_crd) {
        sysc_err!(Code::InvArgs, "Selectors {} already in use", dst_crd);
    }
    if sessions == 0 {
        sysc_err!(Code::InvArgs, "Invalid session count");
    }

    let srvcap: Rc<ServObject> = get_kobj!(vpe, srv_sel, Serv);

    let smsg = kif::service::DeriveCreator {
        opcode: kif::service::Operation::DERIVE_CRT.val as u64,
        sessions: sessions as u64,
    };

    let res = {
        let label = srvcap.creator() as tcu::Label;
        klog!(
            SERV,
            "Sending DERIVE_CRT(sessions={}) to service {} with creator {}",
            sessions,
            srvcap.service().name(),
            label,
        );
        Service::send_receive(srvcap.service(), label, util::object_to_bytes(&smsg))
    };

    match res {
        Err(e) => sysc_err!(e.code(), "Service {} unreachable", srvcap.service().name()),

        Ok(rmsg) => {
            let reply: &kif::service::DeriveCreatorReply = get_message(rmsg);
            let res = reply.res;
            let creator = reply.creator as usize;
            let sgate_sel = reply.sgate_sel as CapSel;

            sysc_log!(
                vpe,
                "derive_srv continue with res={}, creator={}",
                res,
                creator
            );

            if res != 0 {
                sysc_err!(Code::from(res as u32), "Server denied session derivation");
            }

            // derive new service object
            {
                let cap = Capability::new(
                    dst_crd.start() + 0,
                    KObject::Serv(ServObject::new(srvcap.service().clone(), false, creator)),
                );
                vpe.obj_caps().borrow_mut().insert_as_child(cap, srv_sel);
            }

            // obtain SendGate from server
            {
                let serv_vpe = srvcap.service().vpe();
                let mut serv_caps = serv_vpe.obj_caps().borrow_mut();
                let src_cap = serv_caps.get_mut(sgate_sel);
                match src_cap {
                    None => sysc_err!(Code::InvArgs, "Service gave invalid SendGate cap"),
                    Some(c) => vpe
                        .obj_caps()
                        .borrow_mut()
                        .obtain(dst_crd.start() + 1, c, true),
                }
            }
        },
    }

    reply_success(msg);
    Ok(())
}

#[inline(never)]
fn get_sess(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::GetSession = get_message(msg);
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

    let vpecap: Rc<VPE> = get_kobj!(vpe, vpe_sel, VPE);
    if !vpecap.obj_caps().borrow().unused(dst_sel) {
        sysc_err!(Code::InvArgs, "Selector {} already in use", dst_sel);
    }

    // get service cap
    let mut vpe_caps = vpe.obj_caps().borrow_mut();
    let srvcap = vpe_caps.get_mut(srv_sel).ok_or(Error::new(Code::InvArgs))?;
    let creator = match srvcap.get() {
        KObject::Serv(s) => s.creator(),
        _ => sysc_err!(Code::InvArgs, "Expected Serv cap"),
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
            sysc_err!(Code::InvArgs, "Cannot get access to foreign session");
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

fn do_exchange(
    vpe1: &Rc<VPE>,
    vpe2: &Rc<VPE>,
    c1: &kif::CapRngDesc,
    c2: &kif::CapRngDesc,
    obtain: bool,
) -> Result<(), Error> {
    let src = if obtain { vpe2 } else { vpe1 };
    let dst = if obtain { vpe1 } else { vpe2 };
    let src_rng = if obtain { c2 } else { c1 };
    let dst_rng = if obtain { c1 } else { c2 };

    if vpe1.id() == vpe2.id() {
        return Err(Error::new(Code::InvArgs));
    }
    if c1.cap_type() != c2.cap_type() {
        return Err(Error::new(Code::InvArgs));
    }
    if (obtain && c2.count() > c1.count()) || (!obtain && c2.count() != c1.count()) {
        return Err(Error::new(Code::InvArgs));
    }
    if !dst.obj_caps().borrow().range_unused(dst_rng) {
        return Err(Error::new(Code::InvArgs));
    }

    for i in 0..c2.count() {
        let src_sel = src_rng.start() + i;
        let dst_sel = dst_rng.start() + i;
        let mut obj_caps_ref = src.obj_caps().borrow_mut();
        let src_cap = obj_caps_ref.get_mut(src_sel);
        src_cap.map(|c| dst.obj_caps().borrow_mut().obtain(dst_sel, c, true));
    }

    Ok(())
}

#[inline(never)]
fn exchange(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::Exchange = get_message(msg);
    let vpe_sel = req.vpe_sel as CapSel;
    let own_crd = CapRngDesc::new_from(req.own_crd);
    let other_crd = CapRngDesc::new(own_crd.cap_type(), req.other_sel as CapSel, own_crd.count());
    let obtain = req.obtain == 1;

    sysc_log!(
        vpe,
        "exchange(vpe={}, own={}, other={}, obtain={})",
        vpe_sel,
        own_crd,
        other_crd,
        obtain
    );

    let vpe_ref: Rc<VPE> = get_kobj!(vpe, vpe_sel, VPE);

    do_exchange(vpe, &vpe_ref, &own_crd, &other_crd, obtain)?;

    reply_success(msg);
    Ok(())
}

#[inline(never)]
fn exchange_over_sess(
    vpe: &Rc<VPE>,
    msg: &'static tcu::Message,
    obtain: bool,
) -> Result<(), SyscError> {
    let req: &kif::syscalls::ExchangeSess = get_message(msg);
    let vpe_sel = req.vpe_sel as CapSel;
    let sess_sel = req.sess_sel as CapSel;
    let crd = CapRngDesc::new_from(req.crd);

    sysc_log!(
        vpe,
        "{}(vpe={}, sess={}, crd={})",
        if obtain { "obtain" } else { "delegate" },
        vpe_sel,
        sess_sel,
        crd
    );

    let vpecap: Rc<VPE> = get_kobj!(vpe, vpe_sel, VPE);
    let sess: Rc<SessObject> = get_kobj!(vpe, sess_sel, Sess);

    let smsg = kif::service::Exchange {
        opcode: if obtain {
            kif::service::Operation::OBTAIN.val as u64
        }
        else {
            kif::service::Operation::DELEGATE.val as u64
        },
        sess: sess.ident(),
        data: kif::service::ExchangeData {
            caps: crd.count() as u64,
            args: req.args.clone(),
        },
    };

    let serv: Rc<ServObject> = sess.service().clone();
    let label = sess.creator() as tcu::Label;

    klog!(
        SERV,
        "Sending {}(sess={:#x}, caps={}, args={}B) to service {} with creator {}",
        if obtain { "OBTAIN" } else { "DELEGATE" },
        sess.ident(),
        crd.count(),
        { req.args.bytes },
        serv.service().name(),
        label,
    );
    let res = Service::send_receive(serv.service(), label, util::object_to_bytes(&smsg));

    match res {
        Err(e) => sysc_err!(e.code(), "Service {} unreachable", serv.service().name()),

        Ok(rmsg) => {
            let reply: &kif::service::ExchangeReply = get_message(rmsg);

            sysc_log!(
                vpe,
                "{} continue with res={}",
                if obtain { "obtain" } else { "delegate" },
                { reply.res }
            );

            if reply.res != 0 {
                sysc_err!(Code::from(reply.res as u32), "Server denied cap exchange");
            }
            else {
                let err = do_exchange(
                    &vpecap,
                    &serv.service().vpe(),
                    &crd,
                    &CapRngDesc::new_from(reply.data.caps),
                    obtain,
                );
                // TODO improve that
                if let Err(e) = err {
                    sysc_err!(e.code(), "Cap exchange failed");
                }
            }

            let kreply = kif::syscalls::ExchangeSessReply {
                error: 0,
                args: reply.data.args.clone(),
            };
            send_reply(msg, &kreply);
        },
    }

    Ok(())
}

#[inline(never)]
fn activate(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::Activate = get_message(msg);
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
                pemux.invalidate_ep(epid, false)?;
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
                let pe_id = m.borrow().pe_id();
                if let Err(e) =
                    pemux.config_mem_ep(epid, vpe_ref.borrow().id(), &m.borrow(), pe_id, rbuf_off)
                {
                    sysc_err!(e.code(), "Unable to configure mem EP");
                }
            },

            KObject::SGate(ref s) => {
                let rgate: Rc<RefCell<RGateObject>> = s.borrow().rgate().clone();

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
                let mut rgate = r.borrow_mut();
                if rgate.activated() {
                    sysc_err!(Code::InvArgs, "Receive gate is already activated");
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
        pemux.invalidate_ep(epid, true)?;
    }

    reply_success(msg);
    Ok(())
}

#[inline(never)]
fn sem_ctrl(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::SemCtrl = get_message(msg);
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
fn vpe_ctrl(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::VPECtrl = get_message(msg);
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

    let vpe_ref: Rc<VPE> = get_kobj!(vpe, vpe_sel, VPE);

    match op {
        kif::syscalls::VPEOp::INIT => {
            vpe_ref.set_mem_base(arg as goff);
            Loader::get().finish_start(&vpe_ref)?;
        },

        kif::syscalls::VPEOp::START => {
            if Rc::ptr_eq(&vpe, &vpe_ref) {
                sysc_err!(Code::InvArgs, "VPE can't start itself");
            }

            VPE::start_app(&vpe_ref, arg as i32)?;
        },

        kif::syscalls::VPEOp::STOP => {
            let is_self = vpe_sel == kif::SEL_VPE;
            VPE::stop_app(&vpe_ref, arg as i32, is_self);
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
fn vpe_wait(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::VPEWait = get_message(msg);
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

#[inline(never)]
fn revoke(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    let req: &kif::syscalls::Revoke = get_message(msg);
    let vpe_sel = req.vpe_sel as CapSel;
    let crd = CapRngDesc::new_from(req.crd);
    let own = req.own == 1;

    sysc_log!(vpe, "revoke(vpe={}, crd={}, own={})", vpe_sel, crd, own);

    if crd.cap_type() == CapType::OBJECT && crd.start() <= kif::SEL_VPE {
        sysc_err!(Code::InvArgs, "Cap 0, 1, and 2 are not revokeable");
    }

    let kobj = match vpe.obj_caps().borrow().get(vpe_sel) {
        Some(c) => c.get().clone(),
        None => sysc_err!(Code::InvArgs, "Invalid capability"),
    };

    if let KObject::VPE(ref v) = kobj {
        VPE::revoke(v, crd, own);
    }
    else {
        sysc_err!(Code::InvArgs, "Invalid capability");
    }

    reply_success(msg);
    Ok(())
}

fn noop(vpe: &Rc<VPE>, msg: &'static tcu::Message) -> Result<(), SyscError> {
    sysc_log!(vpe, "noop()",);

    reply_success(msg);
    Ok(())
}
