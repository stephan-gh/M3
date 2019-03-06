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
use m3::cell::{RefCell, StaticCell};
use m3::com::{RecvGate, SendGate, SGateArgs};
use m3::col::{String, Treap, Vec};
use m3::errors::{Code, Error};
use m3::kif::{CapRngDesc, CapType};
use m3::rc::Rc;
use m3::session::ResMng;
use m3::syscalls;
use m3::vpe::{Activity, ExecActivity, VPE, VPEArgs};

use boot;
use loader;
use services;

pub type Id = u32;

pub struct Session {
    pub sel: Selector,
    pub ident: u64,
    pub serv: Id,
}

impl Session {
    pub fn new(sel: Selector, ident: u64, serv: Id) -> Self {
        Session {
            sel: sel,
            ident: ident,
            serv: serv,
        }
    }
}

pub struct Resources {
    pub childs: Vec<(Id, Selector)>,
    pub services: Vec<(Id, Selector)>,
    pub sessions: Vec<Session>,
}

impl Resources {
    pub fn new() -> Self {
        Resources {
            childs: Vec::new(),
            services: Vec::new(),
            sessions: Vec::new(),
        }
    }
}

pub trait Child {
    fn id(&self) -> Id;
    fn name(&self) -> &String;
    fn daemon(&self) -> bool;
    fn foreign(&self) -> bool;

    fn vpe_sel(&self) -> Selector;

    fn res(&self) -> &Resources;
    fn res_mut(&mut self) -> &mut Resources;

    fn child_mut(&mut self, vpe_sel: Selector) -> Option<&mut (Child + 'static)> {
        if let Some((id, _)) = self.res_mut().childs.iter().find(|c| c.1 == vpe_sel) {
            get().child_by_id_mut(*id)
        }
        else {
            None
        }
    }

    fn add_child(&mut self, vpe_sel: Selector, rgate: &RecvGate,
                 sgate_sel: Selector, name: String) -> Result<(), Error> {
        let our_sel = self.obtain(vpe_sel)?;
        let child_name = format!("{}.{}", self.name(), name);
        let id = get().next_id();

        log!(ROOT, "{}: add_child(vpe={}, name={}) -> child(id={}, name={})",
             self.name(), vpe_sel, name, id, child_name);

        if self.res().childs.iter().find(|c| c.1 == vpe_sel).is_some() {
            return Err(Error::new(Code::Exists));
        }

        let sgate = SendGate::new_with(SGateArgs::new(&rgate).credits(256).label(id as u64))?;
        let our_sg_sel = sgate.sel();
        let child = Box::new(ForeignChild::new(id, child_name, our_sel, sgate));
        child.delegate(our_sg_sel, sgate_sel)?;
        self.res_mut().childs.push((id, vpe_sel));
        get().add(child);
        Ok(())
    }

    fn rem_child(&mut self, vpe_sel: Selector) -> Result<(), Error> {
        log!(ROOT, "{}: rem_child(vpe={})", self.name(), vpe_sel);

        let idx = self.res().childs.iter().position(|c| c.1 == vpe_sel).ok_or(Error::new(Code::InvArgs))?;
        get().remove_rec(self.res().childs[idx].0);
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

    fn add_service(&mut self, id: Id, sel: Selector) {
        self.res_mut().services.push((id, sel));
    }
    fn has_service(&self, sel: Selector) -> bool {
        self.res().services.iter().find(|t| t.1 == sel).is_some()
    }
    fn remove_service(&mut self, sel: Selector) -> Result<Id, Error> {
        let serv = &mut self.res_mut().services;
        let idx = serv.iter().position(|t| t.1 == sel).ok_or(Error::new(Code::InvArgs))?;
        Ok(serv.remove(idx).0)
    }

    fn add_session(&mut self, sel: Selector, ident: u64, serv: Id) {
        self.res_mut().sessions.push(Session::new(sel, ident, serv));
    }
    fn get_session(&self, sel: Selector) -> Option<&Session> {
        self.res().sessions.iter().find(|s| s.sel == sel)
    }
    fn remove_session(&mut self, sel: Selector) {
        self.res_mut().sessions.retain(|s| s.sel != sel);
    }
}

pub struct BootChild {
    id: Id,
    name: String,
    args: Vec<String>,
    pub reqs: Vec<String>,
    res: Resources,
    daemon: bool,
    activity: Option<ExecActivity>,
    mapper: Option<loader::BootMapper>,
}

impl BootChild {
    pub fn new(id: Id, name: String, args: Vec<String>, reqs: Vec<String>, daemon: bool) -> Self {
        BootChild {
            id: id,
            name: name,
            args: args,
            reqs: reqs,
            res: Resources::new(),
            daemon: daemon,
            activity: None,
            mapper: None,
        }
    }

    pub fn start(&mut self, rgate: &RecvGate, bsel: Selector,
                 m: &'static boot::Mod) -> Result<(), Error> {
        let sgate = SendGate::new_with(SGateArgs::new(&rgate).credits(256).label(self.id as u64))?;
        let vpe = VPE::new_with(VPEArgs::new(&self.name).resmng(ResMng::new(sgate)))?;

        log!(ROOT, "Starting boot module '{}' with arguments {:?}", self.name, &self.args[1..]);

        let bfile = loader::BootFile::new(bsel, m.size as usize);
        let mut bmapper = loader::BootMapper::new(vpe.sel(), bsel, vpe.pe().has_virtmem());
        let bfileref = VPE::cur().files().add(Rc::new(RefCell::new(bfile)))?;
        self.activity = Some(vpe.exec_file(&mut bmapper, bfileref, &self.args)?);
        self.mapper = Some(bmapper);

        Ok(())
    }

    pub fn has_unmet_reqs(&self) -> bool {
        for req in &self.reqs {
            if services::get().get(req).is_err() {
                return true;
            }
        }
        false
    }
}

impl Child for BootChild {
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

    fn res(&self) -> &Resources {
        &self.res
    }
    fn res_mut(&mut self) -> &mut Resources {
        &mut self.res
    }
}

impl Drop for BootChild {
    fn drop(&mut self) {
        while self.res.sessions.len() > 0 {
            let sess = self.res.sessions.remove(0);
            services::get().close_session(self, sess.sel).ok();
        }
    }
}

pub struct ForeignChild {
    id: Id,
    name: String,
    res: Resources,
    vpe: Selector,
    _sgate: SendGate,
}

impl ForeignChild {
    pub fn new(id: Id, name: String, vpe: Selector, sgate: SendGate) -> Self {
        ForeignChild {
            id: id,
            name: name,
            res: Resources::new(),
            vpe: vpe,
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

    fn res(&self) -> &Resources {
        &self.res
    }
    fn res_mut(&mut self) -> &mut Resources {
        &mut self.res
    }
}

pub struct ChildManager {
    childs: Treap<Id, Box<Child>>,
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
            childs: Treap::new(),
            ids: Vec::new(),
            next_id: 0,
            daemons: 0,
            foreigns: 0,
        }
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

    pub fn add(&mut self, child: Box<Child>) {
        if child.daemon() {
            self.daemons += 1;
        }
        if child.foreign() {
            self.foreigns += 1;
        }
        self.next_id = child.id() + 1;
        self.ids.push(child.id());
        self.childs.insert(child.id(), child);
    }

    pub fn child_by_id(&self, id: Id) -> Option<&Child> {
        self.childs.get(&id).map(|c| c.as_ref())
    }
    pub fn child_by_id_mut(&mut self, id: Id) -> Option<&mut (Child + 'static)> {
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

    pub fn kill_child(&mut self, sel: Selector, exitcode: i32) {
        let id = self.sel_to_id(sel);
        let child = self.remove_rec(id);

        log!(ROOT, "Child '{}' exited with exitcode {}", child.name(), exitcode);
    }

    fn remove_rec(&mut self, id: Id) -> Box<Child> {
        let child = self.childs.remove(&id).unwrap();
        self.ids.retain(|&i| i != id);
        if child.daemon() {
            self.daemons -= 1;
        }
        if child.foreign() {
            self.foreigns -= 1;
        }

        log!(ROOT, "Removed child '{}'", child.name());

        for csel in &child.res().childs {
            self.remove_rec(csel.0);
        }
        child
    }

    fn sel_to_id(&self, sel: Selector) -> Id {
        *self.ids.iter().find(|&&id| {
            let child = self.child_by_id(id).unwrap();
            child.vpe_sel() == sel
        }).unwrap()
    }
}
