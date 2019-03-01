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

use m3::cap::Selector;
use m3::cell::{RefCell, StaticCell};
use m3::com::{RecvGate, SendGate, SGateArgs};
use m3::col::{String, Treap, Vec};
use m3::errors::Error;
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
    pub serv: String,
}

impl Session {
    pub fn new(sel: Selector, ident: u64, serv: String) -> Self {
        Session {
            sel: sel,
            ident: ident,
            serv: serv,
        }
    }
}

pub struct Child {
    pub id: Id,
    pub name: String,
    args: Vec<String>,
    pub reqs: Vec<String>,
    sessions: Vec<Session>,
    daemon: bool,
    activity: Option<ExecActivity>,
    mapper: Option<loader::BootMapper>,
}

impl Child {
    pub fn new(id: Id, name: String, args: Vec<String>, reqs: Vec<String>, daemon: bool) -> Self {
        Child {
            id: id,
            name: name,
            args: args,
            reqs: reqs,
            sessions: Vec::new(),
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

    pub fn vpe(&self) -> &VPE {
        self.activity.as_ref().unwrap().vpe()
    }
    pub fn vpe_mut(&mut self) -> &mut VPE {
        self.activity.as_mut().unwrap().vpe_mut()
    }

    pub fn add_session(&mut self, sel: Selector, ident: u64, serv: String) {
        self.sessions.push(Session::new(sel, ident, serv));
    }
    pub fn get_session(&self, sel: Selector) -> Option<&Session> {
        self.sessions.iter().find(|s| s.sel == sel)
    }
    pub fn remove_session(&mut self, sel: Selector) {
        self.sessions.retain(|s| s.sel != sel);
    }
}

impl Drop for Child {
    fn drop(&mut self) {
        while self.sessions.len() > 0 {
            let sess = self.sessions.remove(0);
            services::get().close_session(self, sess.sel).ok();
        }
    }
}

pub struct ChildManager {
    childs: Treap<Id, Child>,
    ids: Vec<Id>,
    daemons: usize,
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
            daemons: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }
    pub fn daemons(&self) -> usize {
        self.daemons
    }

    pub fn add(&mut self, child: Child) -> &mut Child {
        if child.daemon {
            self.daemons += 1;
        }
        self.ids.push(child.id);
        self.childs.insert(child.id, child)
    }

    pub fn child_by_id(&self, id: Id) -> Option<&Child> {
        self.childs.get(&id)
    }
    pub fn child_by_id_mut(&mut self, id: Id) -> Option<&mut Child> {
        self.childs.get_mut(&id)
    }

    pub fn start_waiting(&mut self, event: u64) {
        let mut sels = Vec::new();
        for id in &self.ids {
            let child = self.child_by_id(*id).unwrap();
            sels.push(child.vpe().sel());
        }

        syscalls::vpe_wait(&sels, event).unwrap();
    }

    pub fn kill_child(&mut self, sel: Selector, exitcode: i32) {
        let id = self.sel_to_id(sel);
        let child = self.childs.remove(&id).unwrap();
        self.ids.retain(|&i| i != id);
        if child.daemon {
            self.daemons -= 1;
        }

        log!(ROOT, "Child '{}' exited with exitcode {}", child.name, exitcode);
    }

    fn sel_to_id(&self, sel: Selector) -> Id {
        *self.ids.iter().find(|&&id| {
            let child = self.child_by_id(id).unwrap();
            child.vpe().sel() == sel
        }).unwrap()
    }
}
