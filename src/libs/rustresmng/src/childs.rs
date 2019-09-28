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

use m3::boxed::Box;
use m3::cap::Selector;
use m3::cell::StaticCell;
use m3::col::{String, ToString, Treap, Vec};
use m3::com::{RecvGate, SGateArgs, SendGate};
use m3::dtu;
use m3::errors::{Code, Error};
use m3::kif::{self, CapRngDesc, CapType, Perm};
use m3::rc::Rc;
use m3::syscalls;
use m3::vfs::FileRef;
use m3::vpe::{Activity, ExecActivity, KMem, Mapper, VPE};

use config::Config;
use memory::Allocation;
use sems;
use services::{self, Session};

pub type Id = u32;

pub struct Resources {
    childs: Vec<(Id, Selector)>,
    services: Vec<(Id, Selector)>,
    sessions: Vec<Session>,
    mem: Vec<Allocation>,
}

impl Default for Resources {
    fn default() -> Self {
        Resources {
            childs: Vec::new(),
            services: Vec::new(),
            sessions: Vec::new(),
            mem: Vec::new(),
        }
    }
}

pub trait Child {
    fn id(&self) -> Id;
    fn name(&self) -> &String;
    fn daemon(&self) -> bool;
    fn foreign(&self) -> bool;

    fn vpe_sel(&self) -> Selector;

    fn cfg(&self) -> Rc<Config>;
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
            RESMNG_CHILD,
            "{}: add_child(vpe={}, name={}, sgate_sel={}) -> child(id={}, name={})",
            self.name(),
            vpe_sel,
            name,
            sgate_sel,
            id,
            child_name
        );

        let cfg = self.cfg();
        let cdesc = cfg.get_child(&name);
        let child_cfg = if let Some(cd) = cdesc {
            if cd.is_used() {
                return Err(Error::new(Code::Exists));
            }
            cd.config()
        }
        else {
            cfg.clone()
        };

        if self.res().childs.iter().any(|c| c.1 == vpe_sel) {
            return Err(Error::new(Code::Exists));
        }

        let sgate = SendGate::new_with(SGateArgs::new(&rgate).credits(256).label(u64::from(id)))?;
        let our_sg_sel = sgate.sel();
        let child = Box::new(ForeignChild::new(id, child_name, our_sel, sgate, child_cfg));
        child.delegate(our_sg_sel, sgate_sel)?;

        if let Some(cd) = cdesc {
            cd.mark_used(vpe_sel);
        }
        self.res_mut().childs.push((id, vpe_sel));
        get().add(child);
        Ok(())
    }

    fn rem_child(&mut self, vpe_sel: Selector) -> Result<Id, Error> {
        log!(RESMNG_CHILD, "{}: rem_child(vpe={})", self.name(), vpe_sel);

        let idx = self
            .res()
            .childs
            .iter()
            .position(|c| c.1 == vpe_sel)
            .ok_or_else(|| Error::new(Code::InvArgs))?;
        let id = self.res().childs[idx].0;
        get().remove_rec(id);
        self.cfg().remove_child(vpe_sel);
        self.res_mut().childs.remove(idx);
        Ok(id)
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

    fn add_service(&mut self, id: Id, sel: Selector) {
        self.res_mut().services.push((id, sel));
    }
    fn has_service(&self, sel: Selector) -> bool {
        self.res().services.iter().any(|t| t.1 == sel)
    }
    fn remove_service(&mut self, sel: Selector) -> Result<Id, Error> {
        let serv = &mut self.res_mut().services;
        let idx = serv
            .iter()
            .position(|t| t.1 == sel)
            .ok_or_else(|| Error::new(Code::InvArgs))?;
        Ok(serv.remove(idx).0)
    }

    fn add_session(&mut self, sess: Session) {
        self.res_mut().sessions.push(sess);
    }
    fn get_session(&self, sel: Selector) -> Option<&Session> {
        self.res().sessions.iter().find(|s| s.sel() == sel)
    }
    fn remove_session(&mut self, sel: Selector) -> Result<Session, Error> {
        let sessions = &mut self.res_mut().sessions;
        let idx = sessions
            .iter()
            .position(|s| s.sel() == sel)
            .ok_or_else(|| Error::new(Code::InvArgs))?;
        Ok(sessions.remove(idx))
    }

    fn add_mem(&mut self, alloc: Allocation, mem_sel: Selector, perm: Perm) -> Result<(), Error> {
        log!(RESMNG_MEM, "{}: added {:?}", self.name(), alloc);

        if mem_sel != 0 {
            assert!(alloc.sel != 0);
            syscalls::derive_mem(
                self.vpe_sel(),
                alloc.sel,
                mem_sel,
                alloc.addr,
                alloc.size,
                perm,
            )?;
        }
        self.res_mut().mem.push(alloc);
        Ok(())
    }
    fn remove_mem(&mut self, sel: Selector) -> Result<(), Error> {
        let idx = self
            .res_mut()
            .mem
            .iter()
            .position(|s| s.sel == sel)
            .ok_or_else(|| Error::new(Code::InvArgs))?;
        self.remove_mem_by_idx(idx);
        Ok(())
    }
    fn remove_mem_by_idx(&mut self, idx: usize) {
        let alloc = self.res_mut().mem.remove(idx);
        if alloc.sel != 0 {
            let crd = CapRngDesc::new(CapType::OBJECT, alloc.sel, 1);
            syscalls::revoke(self.vpe_sel(), crd, true).unwrap();
        }

        log!(RESMNG_MEM, "{}: removed {:?}", self.name(), alloc);
    }

    fn use_sem(&mut self, name: &str, sel: Selector) -> Result<(), Error> {
        log!(
            RESMNG_SEM,
            "{}: use_sem(name={}, sel={})",
            self.name(),
            name,
            sel
        );

        let cfg = self.cfg();
        let sdesc = cfg.get_sem(name).ok_or_else(|| Error::new(Code::InvArgs))?;

        let our_sel = sems::get().get(sdesc.global_name()).unwrap();
        self.delegate(our_sel, sel)
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
            let serv = services::get().remove_service(id);
            self.cfg().unreg_service(serv.name());
        }

        while !self.res().mem.is_empty() {
            self.remove_mem_by_idx(0);
        }
    }
}

pub struct OwnChild {
    id: Id,
    name: String,
    args: Vec<String>,
    cfg: Rc<Config>,
    res: Resources,
    daemon: bool,
    activity: Option<ExecActivity>,
    kmem: Rc<KMem>,
}

impl OwnChild {
    pub fn new(id: Id, args: Vec<String>, daemon: bool, kmem: Rc<KMem>, cfg: Rc<Config>) -> Self {
        OwnChild {
            id,
            name: cfg.name().to_string(),
            args,
            cfg,
            res: Resources::default(),
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
            RESMNG,
            "Starting boot module '{}' with arguments {:?}",
            self.name(),
            &self.args[1..]
        );

        self.activity = Some(vpe.exec_file(mapper, file, &self.args)?);

        Ok(())
    }

    pub fn has_unmet_reqs(&self) -> bool {
        for sess in self.cfg().sessions() {
            if services::get().get(sess.serv_name()).is_err() {
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

    fn vpe_sel(&self) -> Selector {
        self.activity.as_ref().unwrap().vpe().sel()
    }

    fn cfg(&self) -> Rc<Config> {
        self.cfg.clone()
    }

    fn res(&self) -> &Resources {
        &self.res
    }

    fn res_mut(&mut self) -> &mut Resources {
        &mut self.res
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
    cfg: Rc<Config>,
    res: Resources,
    vpe: Selector,
    _sgate: SendGate,
}

impl ForeignChild {
    pub fn new(id: Id, name: String, vpe: Selector, sgate: SendGate, cfg: Rc<Config>) -> Self {
        ForeignChild {
            id,
            name,
            cfg,
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

    fn vpe_sel(&self) -> Selector {
        self.vpe
    }

    fn cfg(&self) -> Rc<Config> {
        self.cfg.clone()
    }

    fn res(&self) -> &Resources {
        &self.res
    }

    fn res_mut(&mut self) -> &mut Resources {
        &mut self.res
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

    pub fn next_id(&self) -> Id {
        self.next_id
    }

    pub fn set_next_id(&mut self, id: Id) {
        self.next_id = id;
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
            sels.push(child.vpe_sel());
        }

        syscalls::vpe_wait(&sels, event).unwrap();
    }

    pub fn handle_upcall(&mut self, msg: &'static dtu::Message) {
        let slice: &[kif::upcalls::VPEWait] =
            unsafe { &*(&msg.data as *const [u8] as *const [kif::upcalls::VPEWait]) };
        let upcall = &slice[0];

        self.kill_child(upcall.vpe_sel as Selector, upcall.exitcode as i32);

        let reply = kif::syscalls::DefaultReply { error: 0u64 };
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
                    log!(RESMNG_CHILD, "Killing child '{}'", child.name());
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

            log!(RESMNG_CHILD, "Removed child '{}'", child.name());

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
