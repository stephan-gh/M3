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

use bitflags::bitflags;
use core::fmt;
use m3::boxed::Box;
use m3::cap::Selector;
use m3::cell::{Cell, RefCell, StaticCell};
use m3::col::{String, ToString, Treap, Vec};
use m3::com::{MemGate, RecvGate, SGateArgs, SendGate};
use m3::env;
use m3::errors::{Code, Error};
use m3::format;
use m3::goff;
use m3::kif::{self, CapRngDesc, CapType, Perm};
use m3::log;
use m3::math;
use m3::mem::MsgBuf;
use m3::pes::{Activity, ExecActivity, KMem, Mapper, VPE};
use m3::println;
use m3::rc::Rc;
use m3::session::{ResMngVPEInfo, ResMngVPEInfoResult};
use m3::syscalls;
use m3::tcu;
use m3::vfs::FileRef;

use crate::config::AppConfig;
use crate::gates;
use crate::memory::{self, Allocation, MemPool};
use crate::pes;
use crate::sems;
use crate::services::{self, Session};
use crate::subsys::SubsystemBuilder;
use crate::{events, subsys};

pub type Id = u32;

pub struct ChildMem {
    pool: Rc<RefCell<MemPool>>,
    total: goff,
    quota: Cell<goff>,
}

impl ChildMem {
    pub fn new(pool: Rc<RefCell<MemPool>>, quota: goff) -> Rc<Self> {
        Rc::new(Self {
            pool,
            total: quota,
            quota: Cell::new(quota),
        })
    }

    pub fn pool(&self) -> &Rc<RefCell<MemPool>> {
        &self.pool
    }

    fn have_quota(&self, size: goff) -> bool {
        self.quota.get() > size
    }

    fn alloc_mem(&self, size: goff) {
        assert!(self.have_quota(size));
        self.quota.replace(self.quota.get() - size);
    }

    fn free_mem(&self, size: goff) {
        self.quota.replace(self.quota.get() + size);
    }
}

impl fmt::Debug for ChildMem {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "ChildMem[quota={}]", self.quota.get(),)
    }
}

pub struct Resources {
    childs: Vec<(Id, Selector)>,
    services: Vec<(Id, Selector)>,
    sessions: Vec<(usize, Session)>,
    mem: Vec<(Option<Selector>, Allocation)>,
    pes: Vec<(pes::PEUsage, usize, Selector)>,
    sgates: Vec<SendGate>,
}

impl Default for Resources {
    fn default() -> Self {
        Resources {
            childs: Vec::new(),
            services: Vec::new(),
            sessions: Vec::new(),
            mem: Vec::new(),
            pes: Vec::new(),
            sgates: Vec::new(),
        }
    }
}

pub trait Child {
    fn id(&self) -> Id;
    fn layer(&self) -> u32;
    fn name(&self) -> &String;
    fn daemon(&self) -> bool;
    fn foreign(&self) -> bool;

    fn our_pe(&self) -> Rc<pes::PEUsage>;
    fn child_pe(&self) -> Option<Rc<pes::PEUsage>>;
    fn vpe_sel(&self) -> Selector;
    fn vpe_id(&self) -> tcu::VPEId;
    fn resmng_sgate_sel(&self) -> Selector;

    fn subsys(&mut self) -> Option<&mut SubsystemBuilder>;
    fn mem(&self) -> &Rc<ChildMem>;
    fn cfg(&self) -> Rc<AppConfig>;
    fn res(&self) -> &Resources;
    fn res_mut(&mut self) -> &mut Resources;

    fn child_mut(&mut self, vpe_sel: Selector) -> Option<&mut (dyn Child + 'static)> {
        if let Some((id, _)) = self.res_mut().childs.iter().find(|c| c.1 == vpe_sel) {
            get().child_by_id_mut(*id)
        }
        else {
            None
        }
    }

    fn add_child(
        &mut self,
        vpe_id: tcu::VPEId,
        vpe_sel: Selector,
        rgate: &RecvGate,
        sgate_sel: Selector,
        name: String,
    ) -> Result<(), Error> {
        let our_sel = self.obtain(vpe_sel)?;
        let child_name = format!("{}.{}", self.name(), name);
        let id = get().next_id();

        log!(
            crate::LOG_CHILD,
            "{}: add_child(vpe={}, name={}, sgate_sel={}) -> child(id={}, name={})",
            self.name(),
            vpe_sel,
            name,
            sgate_sel,
            id,
            child_name
        );

        if self.res().childs.iter().any(|c| c.1 == vpe_sel) {
            return Err(Error::new(Code::Exists));
        }

        #[allow(clippy::useless_conversion)]
        let sgate = SendGate::new_with(
            SGateArgs::new(&rgate)
                .credits(1)
                .label(tcu::Label::from(id)),
        )?;
        let our_sg_sel = sgate.sel();
        let child = Box::new(ForeignChild::new(
            id,
            self.layer() + 1,
            child_name,
            // actually, we don't know the PE it's running on. But the PEUsage is only used to set
            // the PMP EPs and currently, no child can actually influence these. For that reason,
            // all childs get the same PMP EPs, so that we can also give the same PMP EPs to childs
            // of childs.
            self.our_pe(),
            vpe_id,
            our_sel,
            sgate,
            self.cfg(),
            self.mem().clone(),
        ));
        child.delegate(our_sg_sel, sgate_sel)?;

        self.res_mut().childs.push((id, vpe_sel));
        get().add(child);
        Ok(())
    }

    fn rem_child_async(&mut self, vpe_sel: Selector) -> Result<(), Error> {
        log!(
            crate::LOG_CHILD,
            "{}: rem_child(vpe={})",
            self.name(),
            vpe_sel
        );

        let idx = self
            .res()
            .childs
            .iter()
            .position(|c| c.1 == vpe_sel)
            .ok_or_else(|| Error::new(Code::InvArgs))?;
        let id = self.res().childs[idx].0;
        get().remove_rec_async(id);
        self.res_mut().childs.remove(idx);
        Ok(())
    }

    fn delegate(&self, src: Selector, dst: Selector) -> Result<(), Error> {
        let crd = CapRngDesc::new(CapType::OBJECT, src, 1);
        syscalls::exchange(self.vpe_sel(), crd, dst, false)
    }
    fn obtain(&self, src: Selector) -> Result<Selector, Error> {
        let dst = VPE::cur().alloc_sels(1);
        let own = CapRngDesc::new(CapType::OBJECT, dst, 1);
        syscalls::exchange(self.vpe_sel(), own, src, true)?;
        Ok(dst)
    }

    fn has_service(&self, sel: Selector) -> bool {
        self.res().services.iter().any(|t| t.1 == sel)
    }

    fn reg_service(
        &mut self,
        srv_sel: Selector,
        sgate_sel: Selector,
        name: String,
        sessions: u32,
    ) -> Result<(), Error> {
        log!(
            crate::LOG_SERV,
            "{}: reg_serv(srv_sel={}, sgate_sel={}, name={}, sessions={})",
            self.name(),
            srv_sel,
            sgate_sel,
            name,
            sessions,
        );

        let cfg = self.cfg();
        let sdesc = cfg
            .get_service(&name)
            .ok_or_else(|| Error::new(Code::InvArgs))?;
        if sdesc.is_used() {
            return Err(Error::new(Code::Exists));
        }

        let our_srv = self.obtain(srv_sel)?;
        let our_sgate = self.obtain(sgate_sel)?;
        let id = services::get().add_service(
            self.id(),
            our_srv,
            our_sgate,
            sdesc.name().global().to_string(),
            sessions,
            true,
        )?;

        sdesc.mark_used();
        self.res_mut().services.push((id, srv_sel));

        Ok(())
    }

    fn unreg_service_async(&mut self, sel: Selector, notify: bool) -> Result<(), Error> {
        log!(crate::LOG_SERV, "{}: unreg_serv(sel={})", self.name(), sel);

        let id = {
            let serv = &mut self.res_mut().services;
            serv.iter()
                .position(|t| t.1 == sel)
                .ok_or_else(|| Error::new(Code::InvArgs))
                .map(|idx| serv.remove(idx).0)
        }?;

        let serv = services::get().remove_service_async(id, notify);
        self.cfg().unreg_service(serv.name());

        Ok(())
    }

    fn open_session_async(&mut self, dst_sel: Selector, name: &str) -> Result<(), Error> {
        log!(
            crate::LOG_SERV,
            "{}: open_sess(dst_sel={}, name={})",
            self.name(),
            dst_sel,
            name
        );

        let cfg = self.cfg();
        let (idx, sdesc) = cfg
            .get_session(name)
            .ok_or_else(|| Error::new(Code::InvArgs))?;
        if sdesc.is_used() {
            return Err(Error::new(Code::Exists));
        }

        let serv = services::get().get(sdesc.name().global())?;
        let sess = Session::new_async(self.id(), dst_sel, serv, sdesc.arg())?;
        // check again if it's still unused, because of the async call above
        if sdesc.is_used() {
            return Err(Error::new(Code::Exists));
        }

        syscalls::get_sess(serv.sel(), self.vpe_sel(), dst_sel, sess.ident())?;

        sdesc.mark_used();
        self.res_mut().sessions.push((idx, sess));

        Ok(())
    }

    fn close_session_async(&mut self, sel: Selector) -> Result<(), Error> {
        log!(crate::LOG_SERV, "{}: close_sess(sel={})", self.name(), sel);

        let (cfg_idx, sess) = {
            let sessions = &mut self.res_mut().sessions;
            sessions
                .iter()
                .position(|(_, s)| s.sel() == sel)
                .ok_or_else(|| Error::new(Code::InvArgs))
                .map(|res_idx| sessions.remove(res_idx))
        }?;

        self.cfg().close_session(cfg_idx);
        sess.close_async(self.id())
    }

    fn alloc_local(&mut self, size: goff, perm: Perm) -> Result<MemGate, Error> {
        log!(
            crate::LOG_MEM,
            "{}: allocate_local(size={:#x}, perm={:?})",
            self.name(),
            size,
            perm
        );

        if !self.mem().have_quota(size) {
            return Err(Error::new(Code::NoSpace));
        }

        let alloc = self.mem().pool.borrow_mut().allocate(size)?;
        let mem_sel = self.mem().pool.borrow().mem_cap(alloc.slice_id());
        let mgate = MemGate::new_bind(mem_sel).derive(alloc.addr(), alloc.size() as usize, perm)?;
        // TODO this memory is currently only free'd on child exit
        self.add_mem(alloc, None);
        Ok(mgate)
    }

    fn alloc_mem(&mut self, dst_sel: Selector, size: goff, perm: Perm) -> Result<(), Error> {
        log!(
            crate::LOG_MEM,
            "{}: allocate(dst_sel={}, size={:#x}, perm={:?})",
            self.name(),
            dst_sel,
            size,
            perm
        );

        if !self.mem().have_quota(size) {
            return Err(Error::new(Code::NoSpace));
        }

        let alloc = self.mem().pool.borrow_mut().allocate(size)?;
        let mem_sel = self.mem().pool.borrow().mem_cap(alloc.slice_id());
        self.add_child_mem(alloc, mem_sel, dst_sel, perm)
    }
    fn alloc_mem_at(
        &mut self,
        dst_sel: Selector,
        offset: goff,
        size: goff,
        perm: Perm,
    ) -> Result<(), Error> {
        log!(
            crate::LOG_MEM,
            "{}: allocate_at(dst_sel={}, size={:#x}, offset={:#x}, perm={:?})",
            self.name(),
            dst_sel,
            size,
            offset,
            perm
        );

        let alloc = self
            .mem()
            .pool
            .borrow_mut()
            .allocate_at(offset, size, perm)?;
        let mem_sel = self.mem().pool.borrow().mem_cap(alloc.slice_id());
        self.add_child_mem(alloc, mem_sel, dst_sel, perm)
    }
    fn add_child_mem(
        &mut self,
        alloc: Allocation,
        mem_sel: Selector,
        dst_sel: Selector,
        perm: Perm,
    ) -> Result<(), Error> {
        syscalls::derive_mem(
            self.vpe_sel(),
            dst_sel,
            mem_sel,
            alloc.addr(),
            alloc.size() as usize,
            perm,
        )
        .map_err(|e| {
            self.mem().pool.borrow_mut().free(alloc);
            e
        })?;

        self.add_mem(alloc, Some(dst_sel));
        Ok(())
    }
    fn add_mem(&mut self, alloc: Allocation, dst_sel: Option<Selector>) {
        self.res_mut().mem.push((dst_sel, alloc));
        if !self.mem().pool.borrow().slices()[alloc.slice_id()].in_reserved_mem() {
            self.mem().alloc_mem(alloc.size());
        }
        log!(
            crate::LOG_MEM,
            "{}: added {:?} (quota left: {})",
            self.name(),
            alloc,
            self.mem().quota.get(),
        );
    }

    fn free_mem(&mut self, sel: Selector) -> Result<(), Error> {
        let idx = self
            .res_mut()
            .mem
            .iter()
            .position(|(s, _)| match s {
                Some(s) => *s == sel,
                _ => false,
            })
            .ok_or_else(|| Error::new(Code::InvArgs))?;
        self.remove_mem_by_idx(idx);
        Ok(())
    }
    fn remove_mem_by_idx(&mut self, idx: usize) {
        let (sel, alloc) = self.res_mut().mem.remove(idx);
        if let Some(s) = sel {
            let crd = CapRngDesc::new(CapType::OBJECT, s, 1);
            // ignore failures here; maybe the VPE is already gone
            syscalls::revoke(self.vpe_sel(), crd, true).ok();
        }

        log!(
            crate::LOG_MEM,
            "{}: removed {:?} (quota left: {})",
            self.name(),
            alloc,
            self.mem().quota.get()
        );
        self.mem().pool.borrow_mut().free(alloc);
        if !self.mem().pool.borrow().slices()[alloc.slice_id()].in_reserved_mem() {
            self.mem().free_mem(alloc.size());
        }
    }

    fn use_rgate(&mut self, name: &str, sel: Selector) -> Result<(u32, u32), Error> {
        log!(
            crate::LOG_GATE,
            "{}: use_rgate(name={}, sel={})",
            self.name(),
            name,
            sel
        );

        let cfg = self.cfg();
        let rdesc = cfg
            .get_rgate(name)
            .ok_or_else(|| Error::new(Code::InvArgs))?;

        let rgate = gates::get().get(rdesc.name().global()).unwrap();
        self.delegate(rgate.sel(), sel)?;
        Ok((
            math::next_log2(rgate.size()),
            math::next_log2(rgate.max_msg_size()),
        ))
    }
    fn use_sgate(&mut self, name: &str, sel: Selector) -> Result<(), Error> {
        log!(
            crate::LOG_GATE,
            "{}: use_sgate(name={}, sel={})",
            self.name(),
            name,
            sel
        );

        let cfg = self.cfg();
        let sdesc = cfg
            .get_sgate(name)
            .ok_or_else(|| Error::new(Code::InvArgs))?;
        if sdesc.is_used() {
            return Err(Error::new(Code::Exists));
        }

        let rgate = gates::get().get(sdesc.name().global()).unwrap();

        let sgate = SendGate::new_with(
            SGateArgs::new(rgate)
                .credits(sdesc.credits())
                .label(sdesc.label()),
        )?;
        self.delegate(sgate.sel(), sel)?;

        sdesc.mark_used();
        self.res_mut().sgates.push(sgate);
        Ok(())
    }
    fn use_sem(&mut self, name: &str, sel: Selector) -> Result<(), Error> {
        log!(
            crate::LOG_SEM,
            "{}: use_sem(name={}, sel={})",
            self.name(),
            name,
            sel
        );

        let cfg = self.cfg();
        let sdesc = cfg.get_sem(name).ok_or_else(|| Error::new(Code::InvArgs))?;

        let sem = sems::get()
            .get(sdesc.name().global())
            .ok_or_else(|| Error::new(Code::NotFound))?;
        self.delegate(sem.sel(), sel)
    }

    fn get_serial(&mut self, sel: Selector) -> Result<(), Error> {
        log!(
            crate::LOG_SERIAL,
            "{}: get_serial(sel={})",
            self.name(),
            sel
        );

        let cfg = self.cfg();
        if cfg.alloc_serial() {
            self.delegate(subsys::SERIAL_RGATE_SEL, sel)
        }
        else {
            Err(Error::new(Code::InvArgs))
        }
    }

    fn get_info(&mut self, idx: Option<usize>) -> Result<ResMngVPEInfoResult, Error> {
        if !self.cfg().can_get_info() {
            return Err(Error::new(Code::NoPerm));
        }

        let (parent_num, parent_layer) = if let Some(presmng) = VPE::cur().resmng() {
            match presmng.get_vpe_count() {
                Err(e) if e.code() == Code::NoPerm => (0, 0),
                Err(e) => return Err(e),
                Ok(res) => res,
            }
        }
        else {
            (0, 0)
        };

        let mut own_num = get().ids.len() + 1;
        for id in &get().ids {
            if get().child_by_id_mut(*id).unwrap().subsys().is_some() {
                own_num -= 1;
            }
        }

        if let Some(mut idx) = idx {
            if idx < parent_num {
                Ok(ResMngVPEInfoResult::Info(
                    VPE::cur().resmng().unwrap().get_vpe_info(idx)?,
                ))
            }
            else if idx - parent_num >= own_num {
                Err(Error::new(Code::NotFound))
            }
            else {
                idx -= parent_num;

                // the first is always us
                if idx == 0 {
                    return Ok(ResMngVPEInfoResult::Info(ResMngVPEInfo {
                        id: VPE::cur().id(),
                        layer: parent_layer + 0,
                        name: env::args().next().unwrap().to_string(),
                        daemon: true,
                        total_mem: memory::container().capacity(),
                        avail_mem: memory::container().available(),
                        pe: VPE::cur().pe_id(),
                    }));
                }
                idx -= 1;

                // find the next non-subsystem child
                let vpe = loop {
                    let vpe = get().child_by_id_mut(get().ids[idx]).unwrap();
                    if vpe.subsys().is_none() {
                        break vpe;
                    }
                    idx += 1;
                };

                Ok(ResMngVPEInfoResult::Info(ResMngVPEInfo {
                    id: vpe.vpe_id(),
                    layer: parent_layer + vpe.layer(),
                    name: vpe.name().to_string(),
                    daemon: vpe.daemon(),
                    total_mem: vpe.mem().total,
                    avail_mem: vpe.mem().quota.get(),
                    pe: vpe.our_pe().pe_id(),
                }))
            }
        }
        else {
            let total = own_num + parent_num;
            Ok(ResMngVPEInfoResult::Count((total, self.layer())))
        }
    }

    fn alloc_pe(
        &mut self,
        sel: Selector,
        desc: kif::PEDesc,
    ) -> Result<(tcu::PEId, kif::PEDesc), Error> {
        log!(
            crate::LOG_PES,
            "{}: alloc_pe(sel={}, desc={:?})",
            self.name(),
            sel,
            desc
        );

        let cfg = self.cfg();
        let idx = cfg.get_pe_idx(desc)?;
        let pe_usage = pes::get().find_and_alloc(desc)?;

        // give this PE access to the same memory regions the child's PE has access to
        // TODO later we could allow childs to customize that
        pe_usage.inherit_mem_regions(&self.our_pe())?;

        self.delegate(pe_usage.pe_obj().sel(), sel)?;

        let pe_id = pe_usage.pe_id();
        let desc = pe_usage.pe_obj().desc();
        self.res_mut().pes.push((pe_usage, idx, sel));
        cfg.alloc_pe(idx);

        Ok((pe_id, desc))
    }

    fn free_pe(&mut self, sel: Selector) -> Result<(), Error> {
        log!(crate::LOG_PES, "{}: free_pe(sel={})", self.name(), sel);

        let idx = self
            .res_mut()
            .pes
            .iter()
            .position(|(_, _, psel)| *psel == sel)
            .ok_or_else(|| Error::new(Code::InvArgs))?;
        self.remove_pe_by_idx(idx)?;

        Ok(())
    }

    fn remove_pe_by_idx(&mut self, idx: usize) -> Result<(), Error> {
        let (pe_usage, idx, ep_sel) = self.res_mut().pes.remove(idx);
        log!(
            crate::LOG_PES,
            "{}: removed PE (id={}, sel={})",
            self.name(),
            pe_usage.pe_id(),
            ep_sel
        );

        let cfg = self.cfg();
        let crd = CapRngDesc::new(CapType::OBJECT, ep_sel, 1);
        // TODO if that fails, we need to kill this child because otherwise we don't get the PE back
        syscalls::revoke(self.vpe_sel(), crd, true).ok();
        cfg.free_pe(idx);

        Ok(())
    }

    fn remove_resources_async(&mut self) {
        while !self.res().sessions.is_empty() {
            let (idx, sess) = self.res_mut().sessions.remove(0);
            self.cfg().close_session(idx);
            sess.close_async(self.id()).ok();
        }

        while !self.res().services.is_empty() {
            let (id, _) = self.res_mut().services.remove(0);
            let serv = services::get().remove_service_async(id, false);
            self.cfg().unreg_service(serv.name());
        }

        while !self.res().mem.is_empty() {
            self.remove_mem_by_idx(0);
        }

        while !self.res().pes.is_empty() {
            self.remove_pe_by_idx(0).ok();
        }
    }
}

pub struct OwnChild {
    id: Id,
    // the activity has to be dropped before we drop the PE
    activity: Option<ExecActivity>,
    our_pe: Rc<pes::PEUsage>,
    child_pe: Rc<pes::PEUsage>,
    name: String,
    args: Vec<String>,
    cfg: Rc<AppConfig>,
    mem: Rc<ChildMem>,
    res: Resources,
    sub: Option<SubsystemBuilder>,
    daemon: bool,
    kmem: Rc<KMem>,
}

impl OwnChild {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: Id,
        our_pe: Rc<pes::PEUsage>,
        child_pe: Rc<pes::PEUsage>,
        args: Vec<String>,
        daemon: bool,
        kmem: Rc<KMem>,
        mem: Rc<ChildMem>,
        cfg: Rc<AppConfig>,
        sub: Option<SubsystemBuilder>,
    ) -> Self {
        OwnChild {
            id,
            our_pe,
            child_pe,
            name: cfg.name().to_string(),
            args,
            cfg,
            mem,
            res: Resources::default(),
            sub,
            daemon,
            activity: None,
            kmem,
        }
    }

    pub fn kmem(&self) -> &Rc<KMem> {
        &self.kmem
    }

    pub fn start(&mut self, vpe: VPE, mapper: &mut dyn Mapper, file: FileRef) -> Result<(), Error> {
        log!(
            crate::LOG_DEF,
            "Starting boot module '{}' on PE{} with arguments {:?}",
            self.name(),
            self.child_pe().unwrap().pe_id(),
            &self.args[1..]
        );

        self.activity = Some(vpe.exec_file(mapper, file, &self.args)?);

        Ok(())
    }

    pub fn has_unmet_reqs(&self) -> bool {
        for sess in self.cfg().sessions() {
            if sess.is_dep() && services::get().get(sess.name().global()).is_err() {
                return true;
            }
        }
        for scrt in self.cfg().sess_creators() {
            if services::get().get(scrt.serv_name()).is_err() {
                return true;
            }
        }
        false
    }
}

impl Child for OwnChild {
    fn id(&self) -> Id {
        self.id
    }

    fn layer(&self) -> u32 {
        1
    }

    fn name(&self) -> &String {
        &self.name
    }

    fn daemon(&self) -> bool {
        self.daemon
    }

    fn foreign(&self) -> bool {
        false
    }

    fn our_pe(&self) -> Rc<pes::PEUsage> {
        self.our_pe.clone()
    }

    fn child_pe(&self) -> Option<Rc<pes::PEUsage>> {
        Some(self.child_pe.clone())
    }

    fn vpe_id(&self) -> tcu::VPEId {
        self.activity.as_ref().unwrap().vpe().id()
    }

    fn vpe_sel(&self) -> Selector {
        self.activity.as_ref().unwrap().vpe().sel()
    }

    fn subsys(&mut self) -> Option<&mut SubsystemBuilder> {
        self.sub.as_mut()
    }

    fn resmng_sgate_sel(&self) -> Selector {
        self.activity
            .as_ref()
            .unwrap()
            .vpe()
            .resmng()
            .as_ref()
            .unwrap()
            .sel()
    }

    fn mem(&self) -> &Rc<ChildMem> {
        &self.mem
    }

    fn cfg(&self) -> Rc<AppConfig> {
        self.cfg.clone()
    }

    fn res(&self) -> &Resources {
        &self.res
    }

    fn res_mut(&mut self) -> &mut Resources {
        &mut self.res
    }
}

impl fmt::Debug for OwnChild {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(
            f,
            "OwnChild[id={}, pe={}, args={:?}, kmem=KMem[sel={}, quota={}], mem={:?}]",
            self.id,
            self.child_pe.pe_id(),
            self.args,
            self.kmem.sel(),
            self.kmem.quota().unwrap(),
            self.mem,
        )
    }
}

pub struct ForeignChild {
    id: Id,
    vpe_id: tcu::VPEId,
    layer: u32,
    name: String,
    parent_pe: Rc<pes::PEUsage>,
    cfg: Rc<AppConfig>,
    mem: Rc<ChildMem>,
    res: Resources,
    vpe_sel: Selector,
    _sgate: SendGate,
}

impl ForeignChild {
    pub fn new(
        id: Id,
        layer: u32,
        name: String,
        parent_pe: Rc<pes::PEUsage>,
        vpe_id: tcu::VPEId,
        vpe_sel: Selector,
        sgate: SendGate,
        cfg: Rc<AppConfig>,
        mem: Rc<ChildMem>,
    ) -> Self {
        ForeignChild {
            id,
            layer,
            name,
            parent_pe,
            cfg,
            mem,
            res: Resources::default(),
            vpe_id,
            vpe_sel,
            _sgate: sgate,
        }
    }
}

impl Child for ForeignChild {
    fn id(&self) -> Id {
        self.id
    }

    fn layer(&self) -> u32 {
        self.layer
    }

    fn name(&self) -> &String {
        &self.name
    }

    fn daemon(&self) -> bool {
        false
    }

    fn foreign(&self) -> bool {
        true
    }

    fn our_pe(&self) -> Rc<pes::PEUsage> {
        self.parent_pe.clone()
    }

    fn child_pe(&self) -> Option<Rc<pes::PEUsage>> {
        None
    }

    fn vpe_id(&self) -> tcu::VPEId {
        self.vpe_id
    }

    fn vpe_sel(&self) -> Selector {
        self.vpe_sel
    }

    fn subsys(&mut self) -> Option<&mut SubsystemBuilder> {
        None
    }

    fn resmng_sgate_sel(&self) -> Selector {
        self._sgate.sel()
    }

    fn mem(&self) -> &Rc<ChildMem> {
        &self.mem
    }

    fn cfg(&self) -> Rc<AppConfig> {
        self.cfg.clone()
    }

    fn res(&self) -> &Resources {
        &self.res
    }

    fn res_mut(&mut self) -> &mut Resources {
        &mut self.res
    }
}

impl fmt::Debug for ForeignChild {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "ForeignChild[id={}, mem={:?}]", self.id, self.mem)
    }
}

bitflags! {
    struct Flags : u64 {
        const STARTING = 1;
        const SHUTDOWN = 2;
    }
}

pub struct ChildManager {
    flags: Flags,
    childs: Treap<Id, Box<dyn Child>>,
    ids: Vec<Id>,
    next_id: Id,
    daemons: usize,
    foreigns: usize,
}

static MNG: StaticCell<ChildManager> = StaticCell::new(ChildManager::new());

pub fn get() -> &'static mut ChildManager {
    MNG.get_mut()
}

impl ChildManager {
    pub const fn new() -> Self {
        ChildManager {
            flags: Flags::STARTING,
            childs: Treap::new(),
            ids: Vec::new(),
            next_id: 0,
            daemons: 0,
            foreigns: 0,
        }
    }

    pub fn should_stop(&self) -> bool {
        // don't stop if we didn't have a child yet. this is necessary, because we use derive_srv
        // asynchronously and thus switch to a different thread while starting a subsystem. thus, if
        // the subsystem is the first child, we would stop without waiting without this workaround.
        !self.flags.contains(Flags::STARTING) && self.children() == 0
    }

    pub fn children(&self) -> usize {
        self.ids.len()
    }

    pub fn daemons(&self) -> usize {
        self.daemons
    }

    pub fn foreigns(&self) -> usize {
        self.foreigns
    }

    pub fn next_id(&mut self) -> Id {
        self.next_id
    }

    pub fn alloc_id(&mut self) -> Id {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn add(&mut self, child: Box<dyn Child>) {
        if child.daemon() {
            self.daemons += 1;
        }
        if child.foreign() {
            self.foreigns += 1;
            self.next_id += 1;
        }
        self.ids.push(child.id());
        self.childs.insert(child.id(), child);
        // now that we have a child, we want to stop as soon as we've no childs anymore
        self.flags.remove(Flags::STARTING);
    }

    pub fn child_by_id(&self, id: Id) -> Option<&dyn Child> {
        self.childs.get(&id).map(|c| c.as_ref())
    }

    pub fn child_by_id_mut(&mut self, id: Id) -> Option<&mut (dyn Child + 'static)> {
        self.childs.get_mut(&id).map(|c| c.as_mut())
    }

    pub fn start_waiting(&mut self, event: u64) {
        let mut sels = Vec::new();
        for id in &self.ids {
            let child = self.child_by_id(*id).unwrap();
            // don't wait for foreign childs, because that's the responsibility of its parent
            if !child.foreign() {
                sels.push(child.vpe_sel());
            }
        }

        syscalls::vpe_wait(&sels, event).unwrap();
    }

    pub fn handle_upcall_async(&mut self, msg: &'static tcu::Message) {
        let upcall = msg.get_data::<kif::upcalls::DefaultUpcall>();

        match kif::upcalls::Operation::from(upcall.opcode) {
            kif::upcalls::Operation::VPE_WAIT => self.upcall_wait_vpe_async(msg),
            kif::upcalls::Operation::DERIVE_SRV => self.upcall_derive_srv(msg),
            _ => panic!("Unexpected upcall {}", upcall.opcode),
        }

        let mut reply_buf = MsgBuf::borrow_def();
        reply_buf.set(kif::DefaultReply { error: 0 });
        RecvGate::upcall()
            .reply(&reply_buf, msg)
            .expect("Upcall reply failed");
    }

    fn upcall_wait_vpe_async(&mut self, msg: &'static tcu::Message) {
        let upcall = msg.get_data::<kif::upcalls::VPEWait>();

        self.kill_child_async(upcall.vpe_sel as Selector, upcall.exitcode as i32);

        // wait for the next
        let no_wait_childs = self.daemons() + self.foreigns();
        if !self.flags.contains(Flags::SHUTDOWN) && self.children() == no_wait_childs {
            self.flags.set(Flags::SHUTDOWN, true);
            self.kill_daemons_async();
            services::get().shutdown_async();
        }
        if !self.should_stop() {
            self.start_waiting(1);
        }
    }

    fn upcall_derive_srv(&mut self, msg: &'static tcu::Message) {
        let upcall = msg.get_data::<kif::upcalls::DeriveSrv>();

        thread::ThreadManager::get().notify(upcall.def.event, Some(msg));
    }

    pub fn kill_child_async(&mut self, sel: Selector, exitcode: i32) {
        if let Some(id) = self.sel_to_id(sel) {
            let child = self.remove_rec_async(id).unwrap();

            if exitcode != 0 {
                println!("Child '{}' exited with exitcode {}", child.name(), exitcode);
            }
        }
    }

    fn kill_daemons_async(&mut self) {
        let ids = self.ids.clone();
        for id in ids {
            // kill all daemons that didn't register a service
            let can_kill = {
                let child = self.child_by_id(id).unwrap();
                if child.daemon() && child.res().services.is_empty() {
                    log!(crate::LOG_CHILD, "Killing child '{}'", child.name());
                    true
                }
                else {
                    false
                }
            };

            if can_kill {
                self.remove_rec_async(id).unwrap();
            }
        }
    }

    fn remove_rec_async(&mut self, id: Id) -> Option<Box<dyn Child>> {
        self.childs.remove(&id).map(|mut child| {
            log!(crate::LOG_CHILD, "Removing child '{}'", child.name());

            // let a potential ongoing async. operation fail
            events::remove_child(id);

            // first, revoke the child's SendGate
            syscalls::revoke(
                VPE::cur().sel(),
                CapRngDesc::new(CapType::OBJECT, child.resmng_sgate_sel(), 1),
                true,
            )
            .ok();
            // now remove all potentially pending messages from the child
            #[allow(clippy::useless_conversion)]
            crate::requests::rgate().drop_msgs_with(child.id().into());

            for csel in &child.res().childs {
                self.remove_rec_async(csel.0);
            }
            child.remove_resources_async();

            self.ids.retain(|&i| i != id);
            if child.daemon() {
                self.daemons -= 1;
            }
            if child.foreign() {
                self.foreigns -= 1;
            }

            log!(crate::LOG_CHILD, "Removed child '{}'", child.name());

            child
        })
    }

    fn sel_to_id(&self, sel: Selector) -> Option<Id> {
        self.ids
            .iter()
            .find(|&&id| {
                let child = self.child_by_id(id).unwrap();
                child.vpe_sel() == sel
            })
            .copied()
    }
}
