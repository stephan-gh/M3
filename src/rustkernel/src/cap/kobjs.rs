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

use base::cell::{Cell, Ref, RefCell, RefMut};
use base::errors::{Code, Error};
use base::goff;
use base::kif;
use base::mem::GlobAddr;
use base::rc::{Rc, Weak};
use base::tcu::{EpId, Label, PEId};
use core::fmt;
use core::ptr;
use thread;

use com::Service;
use mem;
use pes::{pemng, State, VPE};

#[derive(Clone)]
pub enum KObject {
    RGate(Rc<RGateObject>),
    SGate(Rc<SGateObject>),
    MGate(Rc<MGateObject>),
    Map(Rc<MapObject>),
    Serv(Rc<ServObject>),
    Sess(Rc<SessObject>),
    Sem(Rc<SemObject>),
    // Only VPEManager owns a VPE (Rc<VPE>). Break cycle here by using Weak
    VPE(Weak<VPE>),
    KMEM(Rc<KMemObject>),
    PE(Rc<PEObject>),
    EP(Rc<EPObject>),
}

impl fmt::Debug for KObject {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            KObject::SGate(s) => write!(f, "{:?}", s),
            KObject::RGate(r) => write!(f, "{:?}", r),
            KObject::MGate(m) => write!(f, "{:?}", m),
            KObject::Map(m) => write!(f, "{:?}", m),
            KObject::Serv(s) => write!(f, "{:?}", s),
            KObject::Sess(s) => write!(f, "{:?}", s),
            KObject::VPE(v) => write!(f, "{:?}", v),
            KObject::Sem(s) => write!(f, "{:?}", s),
            KObject::KMEM(k) => write!(f, "{:?}", k),
            KObject::PE(p) => write!(f, "{:?}", p),
            KObject::EP(e) => write!(f, "{:?}", e),
        }
    }
}

pub struct GateEP {
    ep: Weak<EPObject>,
}

impl GateEP {
    fn new() -> Self {
        Self { ep: Weak::new() }
    }

    pub fn get_ep(&self) -> Option<Rc<EPObject>> {
        self.ep.upgrade()
    }

    pub fn set_ep(&mut self, o: &Rc<EPObject>) {
        self.ep = Rc::downgrade(o);
    }

    pub fn remove_ep(&mut self) {
        self.ep = Weak::new()
    }
}

pub enum GateObject {
    RGate(Rc<RGateObject>),
    SGate(Rc<SGateObject>),
    MGate(Rc<MGateObject>),
}

impl GateObject {
    pub fn set_ep(&self, ep: &Rc<EPObject>) {
        match self {
            Self::RGate(g) => g.gep.borrow_mut().set_ep(ep),
            Self::SGate(g) => g.gep.borrow_mut().set_ep(ep),
            Self::MGate(g) => g.gep.borrow_mut().set_ep(ep),
        }
    }

    pub fn remove_ep(&self) {
        match self {
            Self::RGate(g) => g.gep.borrow_mut().remove_ep(),
            Self::SGate(g) => g.gep.borrow_mut().remove_ep(),
            Self::MGate(g) => g.gep.borrow_mut().remove_ep(),
        }
    }
}

pub struct RGateObject {
    gep: RefCell<GateEP>,
    loc: Cell<Option<(PEId, EpId)>>,
    addr: Cell<goff>,
    order: u32,
    msg_order: u32,
}

impl RGateObject {
    pub fn new(order: u32, msg_order: u32) -> Rc<Self> {
        Rc::new(Self {
            gep: RefCell::from(GateEP::new()),
            loc: Cell::from(None),
            addr: Cell::from(0),
            order,
            msg_order,
        })
    }

    pub fn gate_ep(&self) -> Ref<GateEP> {
        self.gep.borrow()
    }

    pub fn gate_ep_mut(&self) -> RefMut<GateEP> {
        self.gep.borrow_mut()
    }

    pub fn pe(&self) -> Option<PEId> {
        self.loc.get().map(|(pe, _)| pe)
    }

    pub fn ep(&self) -> Option<EpId> {
        self.loc.get().map(|(_, ep)| ep)
    }

    pub fn addr(&self) -> goff {
        self.addr.get()
    }

    pub fn order(&self) -> u32 {
        self.order
    }

    pub fn size(&self) -> usize {
        1 << self.order
    }

    pub fn msg_order(&self) -> u32 {
        self.msg_order
    }

    pub fn msg_size(&self) -> usize {
        1 << self.msg_order
    }

    pub fn activated(&self) -> bool {
        self.addr.get() != 0
    }

    pub fn activate(&self, pe: PEId, ep: EpId, addr: goff) {
        self.loc.replace(Some((pe, ep)));
        self.addr.replace(addr);
    }

    pub fn deactivate(&self) {
        self.addr.set(0);
        self.loc.set(None);
    }

    pub fn get_event(&self) -> thread::Event {
        self as *const Self as thread::Event
    }

    pub fn print_loc(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.loc.get() {
            Some((pe, ep)) => write!(f, "PE{}:EP{}", pe, ep),
            None => write!(f, "?"),
        }
    }
}

impl fmt::Debug for RGateObject {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "RGate[loc=")?;
        self.print_loc(f)?;
        write!(
            f,
            ", addr={:#x}, sz={:#x}, msz={:#x}]",
            self.addr.get(),
            self.size(),
            self.msg_size()
        )
    }
}

pub struct SGateObject {
    gep: RefCell<GateEP>,
    rgate: Rc<RGateObject>,
    label: Label,
    credits: u32,
}

impl SGateObject {
    pub fn new(rgate: &Rc<RGateObject>, label: Label, credits: u32) -> Rc<Self> {
        Rc::new(Self {
            gep: RefCell::from(GateEP::new()),
            rgate: rgate.clone(),
            label,
            credits,
        })
    }

    pub fn gate_ep(&self) -> Ref<GateEP> {
        self.gep.borrow()
    }

    pub fn gate_ep_mut(&self) -> RefMut<GateEP> {
        self.gep.borrow_mut()
    }

    pub fn rgate(&self) -> &Rc<RGateObject> {
        &self.rgate
    }

    pub fn label(&self) -> Label {
        self.label
    }

    pub fn credits(&self) -> u32 {
        self.credits
    }
}

impl fmt::Debug for SGateObject {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SGate[rgate=")?;
        self.rgate.print_loc(f)?;
        write!(f, ", lbl={:#x}, crd={}]", self.label, self.credits)
    }
}

pub struct MGateObject {
    gep: RefCell<GateEP>,
    mem: mem::Allocation,
    perms: kif::Perm,
    derived: bool,
}

impl MGateObject {
    pub fn new(mem: mem::Allocation, perms: kif::Perm, derived: bool) -> Rc<Self> {
        Rc::new(Self {
            gep: RefCell::from(GateEP::new()),
            mem,
            perms,
            derived,
        })
    }

    pub fn gate_ep(&self) -> Ref<GateEP> {
        self.gep.borrow()
    }

    pub fn gate_ep_mut(&self) -> RefMut<GateEP> {
        self.gep.borrow_mut()
    }

    pub fn pe_id(&self) -> PEId {
        self.mem.global().pe()
    }

    pub fn offset(&self) -> goff {
        self.mem.global().offset()
    }

    pub fn addr(&self) -> GlobAddr {
        self.mem.global()
    }

    pub fn size(&self) -> goff {
        self.mem.size()
    }

    pub fn perms(&self) -> kif::Perm {
        self.perms
    }
}

impl Drop for MGateObject {
    fn drop(&mut self) {
        // if it's not derived, it's always memory from mem-PEs
        if self.derived {
            self.mem.claim();
        }
    }
}

impl fmt::Debug for MGateObject {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "MGate[pe={}, addr={:?}, size={:#x}, perm={:?}, der={}]",
            self.pe_id(),
            self.addr(),
            self.size(),
            self.perms,
            self.derived
        )
    }
}

pub struct ServObject {
    serv: Rc<Service>,
    owner: bool,
    creator: usize,
}

impl ServObject {
    pub fn new(serv: Rc<Service>, owner: bool, creator: usize) -> Rc<Self> {
        Rc::new(Self {
            serv,
            owner,
            creator,
        })
    }

    pub fn service(&self) -> &Rc<Service> {
        &self.serv
    }

    pub fn creator(&self) -> usize {
        self.creator
    }
}

impl fmt::Debug for ServObject {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Serv[srv={:?}, owner={}, creator={}]",
            self.serv, self.owner, self.creator
        )
    }
}

pub struct SessObject {
    srv: Rc<ServObject>,
    creator: usize,
    ident: u64,
}

impl SessObject {
    pub fn new(srv: &Rc<ServObject>, creator: usize, ident: u64) -> Rc<Self> {
        Rc::new(Self {
            srv: srv.clone(),
            creator,
            ident,
        })
    }

    pub fn service(&self) -> &Rc<ServObject> {
        &self.srv
    }

    pub fn creator(&self) -> usize {
        self.creator
    }

    pub fn ident(&self) -> u64 {
        self.ident
    }
}

impl fmt::Debug for SessObject {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Sess[service={}, ident={:#x}]",
            self.service().service().name(),
            self.ident
        )
    }
}

pub struct SemObject {
    counter: Cell<u32>,
    waiters: Cell<i32>,
}

impl SemObject {
    pub fn new(counter: u32) -> Rc<Self> {
        Rc::new(Self {
            counter: Cell::from(counter),
            waiters: Cell::from(0),
        })
    }

    pub fn down(sem: &Rc<Self>) -> Result<(), Error> {
        while unsafe { ptr::read_volatile(sem.counter.as_ptr()) } == 0 {
            sem.waiters.set(sem.waiters.get() + 1);
            let event = sem.get_event();
            thread::ThreadManager::get().wait_for(event);
            if unsafe { ptr::read_volatile(sem.waiters.as_ptr()) } == -1 {
                return Err(Error::new(Code::RecvGone));
            }
            sem.waiters.set(sem.waiters.get() - 1);
        }
        sem.counter.set(sem.counter.get() - 1);
        Ok(())
    }

    pub fn up(&self) {
        if self.waiters.get() > 0 {
            thread::ThreadManager::get().notify(self.get_event(), None);
        }
        self.counter.set(self.counter.get() + 1);
    }

    pub fn revoke(&self) {
        if self.waiters.get() > 0 {
            thread::ThreadManager::get().notify(self.get_event(), None);
        }
        self.waiters.set(-1);
    }

    fn get_event(&self) -> thread::Event {
        self as *const Self as thread::Event
    }
}

impl fmt::Debug for SemObject {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Sem[counter={}, waiters={}]",
            self.counter.get(),
            self.waiters.get()
        )
    }
}

pub struct PEObject {
    pe: PEId,
    eps: Cell<u32>,
    vpes: u32,
}

impl PEObject {
    pub fn new(pe: PEId, eps: u32) -> Rc<Self> {
        Rc::new(Self {
            pe,
            eps: Cell::from(eps),
            vpes: 0,
        })
    }

    pub fn pe(&self) -> PEId {
        self.pe
    }

    pub fn eps(&self) -> u32 {
        self.eps.get()
    }

    pub fn vpes(&self) -> u32 {
        self.vpes
    }

    pub fn has_quota(&self, eps: u32) -> bool {
        self.eps.get() >= eps
    }

    pub fn alloc(&self, eps: u32) {
        klog!(
            PES,
            "PE[{}]: allocating {} EPs ({} total)",
            self.pe,
            eps,
            self.eps()
        );
        assert!(self.eps.get() >= eps);
        self.eps.set(self.eps.get() - eps);
    }

    pub fn free(&self, eps: u32) {
        self.eps.set(self.eps.get() + eps);
        klog!(
            PES,
            "PE[{}]: freed {} EPs ({} total)",
            self.pe,
            eps,
            self.eps()
        );
    }
}

impl fmt::Debug for PEObject {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "PE[id={}, eps={}, vpes={}]",
            self.pe,
            self.eps(),
            self.vpes
        )
    }
}

pub struct EPObject {
    is_std: bool,
    gate: RefCell<Option<GateObject>>,
    vpe: Weak<VPE>,
    ep: EpId,
    replies: u32,
    pe: Weak<PEObject>,
}

impl EPObject {
    pub fn new(is_std: bool, vpe: &Rc<VPE>, ep: EpId, replies: u32, pe: &Rc<PEObject>) -> Rc<Self> {
        let ep = Rc::new(Self {
            is_std,
            gate: RefCell::from(None),
            vpe: Rc::downgrade(vpe),
            ep,
            replies,
            pe: Rc::downgrade(pe),
        });
        vpe.add_ep(ep.clone());
        ep
    }

    pub fn pe_id(&self) -> PEId {
        self.pe.upgrade().unwrap().pe()
    }

    pub fn vpe(&self) -> Rc<VPE> {
        self.vpe.upgrade().unwrap()
    }

    pub fn ep(&self) -> EpId {
        self.ep
    }

    pub fn replies(&self) -> u32 {
        self.replies
    }

    pub fn set_gate(&self, g: GateObject) {
        self.gate.replace(Some(g));
    }

    pub fn revoke(ep: &Rc<Self>) {
        if let Some(v) = ep.vpe.upgrade() {
            v.rem_ep(ep);
        }
    }

    pub fn configure(ep: &Rc<Self>, gate: &KObject) {
        // create a gate object from the kobj
        let go = match gate {
            KObject::MGate(g) => GateObject::MGate(g.clone()),
            KObject::RGate(g) => GateObject::RGate(g.clone()),
            KObject::SGate(g) => GateObject::SGate(g.clone()),
            _ => unreachable!(),
        };
        // we tell the gate object its gate object
        go.set_ep(ep);
        // we tell the endpoint its current gate object
        ep.set_gate(go);
    }

    pub fn deconfigure(&self, force: bool) -> Result<bool, Error> {
        let mut invalidated = false;
        if let Some(ref gate) = &*self.gate.borrow() {
            let pe_id = self.pe_id();
            let pemux = pemng::get().pemux(pe_id);

            // invalidate receive and send EPs
            match gate {
                GateObject::RGate(_) | GateObject::SGate(_) => {
                    pemux.invalidate_ep(self.vpe().id(), self.ep, force, false)?;
                    invalidated = true;
                },
                _ => {},
            }

            // deactivate receive gate
            match gate {
                GateObject::RGate(r) => r.deactivate(),
                _ => {},
            }

            // we tell the gate that it's ep is no longer valid
            gate.remove_ep();
        }
        Ok(invalidated)
    }
}

impl Drop for EPObject {
    fn drop(&mut self) {
        if !self.is_std {
            let pe = self.pe.upgrade().unwrap();

            let pemux = pemng::get().pemux(pe.pe);
            pemux.free_eps(self.ep, 1 + self.replies);

            pe.free(1 + self.replies);
        }
    }
}

impl fmt::Debug for EPObject {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "EPMask[vpe={}, ep={}, replies={}, pe={:?}]",
            self.vpe().id(),
            self.ep,
            self.replies,
            self.pe
        )
    }
}
pub struct KMemObject {
    quota: usize,
    left: usize,
}

impl KMemObject {
    pub fn new(quota: usize) -> Rc<Self> {
        Rc::new(Self { quota, left: quota })
    }

    pub fn left(&self) -> usize {
        self.left
    }

    pub fn has_quota(&self, size: usize) -> bool {
        self.left >= size
    }

    pub fn alloc(&self, _size: usize) {
    }

    pub fn free(&self, _size: usize) {
    }
}

impl fmt::Debug for KMemObject {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "KMem[quota={:#x}, left={:#x}]", self.quota, self.left)
    }
}

pub struct MapObject {
    glob: Cell<GlobAddr>,
    flags: Cell<kif::PageFlags>,
    mapped: Cell<bool>,
}

impl MapObject {
    pub fn new(glob: GlobAddr, flags: kif::PageFlags) -> Rc<Self> {
        Rc::new(Self {
            glob: Cell::from(glob),
            flags: Cell::from(flags),
            mapped: Cell::from(false),
        })
    }

    pub fn mapped(&self) -> bool {
        self.mapped.get()
    }

    pub fn global(&self) -> GlobAddr {
        self.glob.get()
    }

    pub fn flags(&self) -> kif::PageFlags {
        self.flags.get()
    }

    pub fn map(
        &self,
        vpe: &VPE,
        virt: goff,
        glob: GlobAddr,
        pages: usize,
        flags: kif::PageFlags,
    ) -> Result<(), Error> {
        let pemux = pemng::get().pemux(vpe.pe_id());
        pemux.map(vpe.id(), virt, glob, pages, flags).and_then(|_| {
            self.glob.replace(glob);
            self.flags.replace(flags);
            self.mapped.set(true);
            Ok(())
        })
    }

    pub fn unmap(&self, vpe: &Rc<VPE>, virt: goff, pages: usize) {
        if vpe.state() != State::DEAD {
            let pemux = pemng::get().pemux(vpe.pe_id());
            pemux.unmap(vpe.id(), virt, pages).unwrap();
        }
    }
}

impl fmt::Debug for MapObject {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Map[glob={:?}, flags={:#x}]",
            self.global(),
            self.flags()
        )
    }
}
