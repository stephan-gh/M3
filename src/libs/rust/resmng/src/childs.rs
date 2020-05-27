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

use core::fmt;
use m3::boxed::Box;
use m3::cap::Selector;
use m3::cell::{RefCell, StaticCell};
use m3::col::{String, ToString, Treap, Vec};
use m3::com::{RecvGate, SGateArgs, SendGate};
use m3::errors::{Code, Error};
use m3::goff;
use m3::kif::{self, CapRngDesc, CapType, Perm};
use m3::pes::{Activity, ExecActivity, KMem, Mapper, VPE};
use m3::rc::Rc;
use m3::syscalls;
use m3::tcu;
use m3::vfs::FileRef;

use config::AppConfig;
use memory::{Allocation, MemPool};
use pes;
use sems;
use services::{self, Session};
use subsys::SubsystemBuilder;

pub type Id = u32;

pub struct Resources {
    childs: Vec<(Id, Selector)>,
    services: Vec<(Id, Selector)>,
    sessions: Vec<Session>,
    mem: Vec<(Selector, Allocation)>,
    pes: Vec<(pes::PEUsage, usize, Selector)>,
}

impl Default for Resources {
    fn default() -> Self {
        Resources {
            childs: Vec::new(),
            services: Vec::new(),
            sessions: Vec::new(),
            mem: Vec::new(),
            pes: Vec::new(),
        }
    }
}

pub trait Child {
    fn id(&self) -> Id;
    fn name(&self) -> &String;
    fn daemon(&self) -> bool;
    fn foreign(&self) -> bool;

    fn pe(&self) -> Option<Rc<pes::PEUsage>>;
    fn vpe_sel(&self) -> Selector;

    fn mem(&mut self) -> &Rc<RefCell<MemPool>>;
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
            child_name,
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

    fn rem_child(&mut self, vpe_sel: Selector) -> Result<(), Error> {
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
        get().remove_rec(id);
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
        let id = services::get().add_service(our_srv, our_sgate, name, sessions, true)?;

        sdesc.mark_used();
        self.res_mut().services.push((id, srv_sel));

        Ok(())
    }

    fn unreg_service(&mut self, sel: Selector, notify: bool) -> Result<(), Error> {
        log!(crate::LOG_SERV, "{}: unreg_serv(sel={})", self.name(), sel);

        let id = {
            let serv = &mut self.res_mut().services;
            serv.iter()
                .position(|t| t.1 == sel)
                .ok_or_else(|| Error::new(Code::InvArgs))
                .map(|idx| serv.remove(idx).0)
        }?;

        let serv = services::get().remove_service(id, notify);
        self.cfg().unreg_service(serv.name());

        Ok(())
    }

    fn open_session(&mut self, dst_sel: Selector, name: &str) -> Result<(), Error> {
        log!(
            crate::LOG_SERV,
            "{}: open_sess(dst_sel={}, name={})",
            self.name(),
            dst_sel,
            name
        );

        let cfg = self.cfg();
        let sdesc = cfg
            .get_session(name)
            .ok_or_else(|| Error::new(Code::InvArgs))?;
        if sdesc.is_used() {
            return Err(Error::new(Code::Exists));
        }

        let serv = services::get().get(sdesc.serv_name())?;
        let sess = Session::new(dst_sel, serv, sdesc.arg())?;

        syscalls::get_sess(serv.sel(), self.vpe_sel(), dst_sel, sess.ident())?;

        sdesc.mark_used(dst_sel);
        self.res_mut().sessions.push(sess);

        Ok(())
    }

    fn close_session(&mut self, sel: Selector) -> Result<(), Error> {
        log!(crate::LOG_SERV, "{}: close_sess(sel={})", self.name(), sel);

        let sess = {
            let sessions = &mut self.res_mut().sessions;
            sessions
                .iter()
                .position(|s| s.sel() == sel)
                .ok_or_else(|| Error::new(Code::InvArgs))
                .map(|idx| sessions.remove(idx))
        }?;

        self.cfg().close_session(sel);
        sess.close()
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

        let alloc = self.mem().borrow_mut().allocate(size)?;
        let mem_sel = self.mem().borrow().mem_cap(alloc.slice_id());
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

        let alloc = self.mem().borrow_mut().allocate_at(offset, size)?;
        let mem_sel = self.mem().borrow().mem_cap(alloc.slice_id());
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
            self.mem().borrow_mut().free(alloc);
            e
        })?;

        self.add_mem(alloc, Some(dst_sel));
        Ok(())
    }
    fn add_mem(&mut self, alloc: Allocation, dst_sel: Option<Selector>) {
        log!(crate::LOG_MEM, "{}: added {:?}", self.name(), alloc);
        self.res_mut().mem.push((dst_sel.unwrap_or(0), alloc));
    }

    fn free_mem(&mut self, sel: Selector) -> Result<(), Error> {
        let idx = self
            .res_mut()
            .mem
            .iter()
            .position(|(s, _)| *s == sel)
            .ok_or_else(|| Error::new(Code::InvArgs))?;
        self.remove_mem_by_idx(idx);
        Ok(())
    }
    fn remove_mem_by_idx(&mut self, idx: usize) {
        let (sel, alloc) = self.res_mut().mem.remove(idx);
        if sel != 0 {
            let crd = CapRngDesc::new(CapType::OBJECT, sel, 1);
            // ignore failures here; maybe the VPE is already gone
            syscalls::revoke(self.vpe_sel(), crd, true).ok();
        }

        log!(crate::LOG_MEM, "{}: removed {:?}", self.name(), alloc);
        self.mem().borrow_mut().free(alloc);
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

        let sem = sems::get().get(sdesc.global_name()).unwrap();
        self.delegate(sem.sel(), sel)
    }

    fn alloc_pe(&mut self, sel: Selector, desc: kif::PEDesc) -> Result<kif::PEDesc, Error> {
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

        self.delegate(pe_usage.pe_obj().sel(), sel)?;

        let desc = pe_usage.pe_obj().desc();
        self.res_mut().pes.push((pe_usage, idx, sel));
        cfg.alloc_pe(idx);

        Ok(desc)
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

    fn remove_resources(&mut self)
    where
        Self: Sized,
    {
        while !self.res().sessions.is_empty() {
            let sess = self.res_mut().sessions.remove(0);
            self.cfg().close_session(sess.sel());
            sess.close().ok();
        }

        while !self.res().services.is_empty() {
            let (id, _) = self.res_mut().services.remove(0);
            let serv = services::get().remove_service(id, false);
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
    pe: Rc<pes::PEUsage>,
    name: String,
    args: Vec<String>,
    cfg: Rc<AppConfig>,
    mem: Rc<RefCell<MemPool>>,
    res: Resources,
    sub: Option<SubsystemBuilder>,
    daemon: bool,
    activity: Option<ExecActivity>,
    kmem: Rc<KMem>,
}

impl OwnChild {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: Id,
        pe: Rc<pes::PEUsage>,
        args: Vec<String>,
        daemon: bool,
        kmem: Rc<KMem>,
        mem: Rc<RefCell<MemPool>>,
        cfg: Rc<AppConfig>,
        sub: Option<SubsystemBuilder>,
    ) -> Self {
        OwnChild {
            id,
            pe,
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

    pub fn subsys(&mut self) -> Option<&mut SubsystemBuilder> {
        self.sub.as_mut()
    }

    pub fn start(&mut self, vpe: VPE, mapper: &mut dyn Mapper, file: FileRef) -> Result<(), Error> {
        log!(
            crate::LOG_DEF,
            "Starting boot module '{}' with arguments {:?}",
            self.name(),
            &self.args[1..]
        );

        self.activity = Some(vpe.exec_file(mapper, file, &self.args)?);

        Ok(())
    }

    pub fn has_unmet_reqs(&self) -> bool {
        for sess in self.cfg().sessions() {
            if sess.is_dep() && services::get().get(sess.serv_name()).is_err() {
                return true;
            }
        }
        for serv in self.cfg().dependencies() {
            if services::get().get(serv).is_err() {
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

    fn name(&self) -> &String {
        &self.name
    }

    fn daemon(&self) -> bool {
        self.daemon
    }

    fn foreign(&self) -> bool {
        false
    }

    fn pe(&self) -> Option<Rc<pes::PEUsage>> {
        Some(self.pe.clone())
    }

    fn vpe_sel(&self) -> Selector {
        self.activity.as_ref().unwrap().vpe().sel()
    }

    fn mem(&mut self) -> &Rc<RefCell<MemPool>> {
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
            self.pe.pe_id(),
            self.args,
            self.kmem.sel(),
            self.kmem.quota().unwrap(),
            self.mem.borrow()
        )
    }
}

impl Drop for OwnChild {
    fn drop(&mut self) {
        self.remove_resources();
    }
}

pub struct ForeignChild {
    id: Id,
    name: String,
    cfg: Rc<AppConfig>,
    mem: Rc<RefCell<MemPool>>,
    res: Resources,
    vpe: Selector,
    _sgate: SendGate,
}

impl ForeignChild {
    pub fn new(
        id: Id,
        name: String,
        vpe: Selector,
        sgate: SendGate,
        cfg: Rc<AppConfig>,
        mem: Rc<RefCell<MemPool>>,
    ) -> Self {
        ForeignChild {
            id,
            name,
            cfg,
            mem,
            res: Resources::default(),
            vpe,
            _sgate: sgate,
        }
    }
}

impl Child for ForeignChild {
    fn id(&self) -> Id {
        self.id
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

    fn pe(&self) -> Option<Rc<pes::PEUsage>> {
        None
    }

    fn vpe_sel(&self) -> Selector {
        self.vpe
    }

    fn mem(&mut self) -> &Rc<RefCell<MemPool>> {
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

impl Drop for ForeignChild {
    fn drop(&mut self) {
        self.remove_resources();
    }
}

pub struct ChildManager {
    childs: Treap<Id, Box<dyn Child>>,
    ids: Vec<Id>,
    next_id: Id,
    daemons: usize,
    foreigns: usize,
    shutdown: bool,
}

static MNG: StaticCell<ChildManager> = StaticCell::new(ChildManager::new());

pub fn get() -> &'static mut ChildManager {
    MNG.get_mut()
}

impl ChildManager {
    pub const fn new() -> Self {
        ChildManager {
            childs: Treap::new(),
            ids: Vec::new(),
            next_id: 0,
            daemons: 0,
            foreigns: 0,
            shutdown: false,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> usize {
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

    pub fn handle_upcall(&mut self, msg: &'static tcu::Message) {
        let upcall = msg.get_data::<kif::upcalls::VPEWait>();

        self.kill_child(upcall.vpe_sel as Selector, upcall.exitcode as i32);

        let reply = kif::DefaultReply { error: 0u64 };
        RecvGate::upcall()
            .reply(&[reply], msg)
            .expect("Upcall reply failed");

        // wait for the next
        let no_wait_childs = self.daemons() + self.foreigns();
        if !self.shutdown && self.len() == no_wait_childs {
            self.shutdown = true;
            self.kill_daemons();
            services::get().shutdown();
        }
        if !self.is_empty() {
            self.start_waiting(1);
        }
    }

    pub fn kill_child(&mut self, sel: Selector, exitcode: i32) {
        if let Some(id) = self.sel_to_id(sel) {
            let child = self.remove_rec(id).unwrap();

            if exitcode != 0 {
                println!("Child '{}' exited with exitcode {}", child.name(), exitcode);
            }
        }
    }

    fn kill_daemons(&mut self) {
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
                self.remove_rec(id).unwrap();
            }
        }
    }

    fn remove_rec(&mut self, id: Id) -> Option<Box<dyn Child>> {
        self.childs.remove(&id).map(|child| {
            self.ids.retain(|&i| i != id);
            if child.daemon() {
                self.daemons -= 1;
            }
            if child.foreign() {
                self.foreigns -= 1;
            }

            log!(crate::LOG_CHILD, "Removed child '{}'", child.name());

            for csel in &child.res().childs {
                self.remove_rec(csel.0);
            }
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
