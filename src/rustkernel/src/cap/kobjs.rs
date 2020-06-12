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
use pes::{pemng, VPEId, VPE};

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

pub struct CommonGateProperties {
    /// The EP where this RGate could be attached to.
    ep: Weak<EPObject>,
}

impl CommonGateProperties {
    fn new() -> CommonGateProperties {
        CommonGateProperties { ep: Weak::new() }
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

/// Enum-Type that combines all GateObjects.
pub enum GateObject {
    RGate(Weak<RGateObject>),
    SGate(Weak<SGateObject>),
    MGate(Weak<MGateObject>),
}

impl GateObject {
    /// If self is a RGATE
    pub fn is_r_gate(&self) -> bool {
        match *self {
            GateObject::RGate(_) => true,
            _ => false,
        }
    }

    /// If self is a SGATE
    pub fn is_s_gate(&self) -> bool {
        match *self {
            GateObject::SGate(_) => true,
            _ => false,
        }
    }

    /// If self is a MGATE
    pub fn is_m_gate(&self) -> bool {
        match *self {
            GateObject::MGate(_) => true,
            _ => false,
        }
    }

    // Returns RGATE if self is RGATE
    pub fn get_r_gate(&self) -> Rc<RGateObject> {
        if let GateObject::RGate(g) = self {
            g.upgrade().unwrap()
        }
        else {
            panic!("No RGateObject!");
        }
    }

    // Returns RGATE if self is RGATE
    pub fn get_s_gate(&self) -> Rc<SGateObject> {
        if let GateObject::SGate(g) = self {
            g.upgrade().unwrap()
        }
        else {
            panic!("No SGateObject!");
        }
    }

    // Returns MGATE if self is MGATE
    pub fn get_m_gate(&self) -> Rc<MGateObject> {
        if let GateObject::MGate(g) = self {
            g.upgrade().unwrap()
        }
        else {
            panic!("No MGateObject!");
        }
    }

    /// Sets the endpoint on the gate. Delegates the action
    /// from this enum to the actual gate.
    pub fn set_ep(&self, ep: &Rc<EPObject>) {
        match self {
            GateObject::RGate(g) => g.upgrade().unwrap().cgp.borrow_mut().set_ep(ep),
            GateObject::SGate(g) => g.upgrade().unwrap().cgp.borrow_mut().set_ep(ep),
            GateObject::MGate(g) => g.upgrade().unwrap().cgp.borrow_mut().set_ep(ep),
        }
    }

    /// Check is the underlying gate object has an endpoint assigned.
    pub fn has_ep(&self) -> bool {
        match self {
            GateObject::RGate(g) => g.upgrade().unwrap().cgp.borrow().get_ep().is_some(),
            GateObject::SGate(g) => g.upgrade().unwrap().cgp.borrow().get_ep().is_some(),
            GateObject::MGate(g) => g.upgrade().unwrap().cgp.borrow().get_ep().is_some(),
        }
    }

    /// Convenient function on the enum to get the EP of the gate.
    pub fn get_ep(&self) -> Rc<EPObject> {
        match self {
            GateObject::RGate(g) => g.upgrade().unwrap().cgp.borrow().get_ep().unwrap().clone(),
            GateObject::SGate(g) => g.upgrade().unwrap().cgp.borrow().get_ep().unwrap().clone(),
            GateObject::MGate(g) => g.upgrade().unwrap().cgp.borrow().get_ep().unwrap().clone(),
        }
    }

    /// Convenient function that delegates this action
    /// to the corresponding gate type.
    pub fn remove_ep(&self) {
        match self {
            GateObject::RGate(g) => {
                if let Some(g) = g.upgrade() {
                    g.cgp.borrow_mut().remove_ep()
                }
            },
            GateObject::SGate(g) => {
                if let Some(g) = g.upgrade() {
                    g.cgp.borrow_mut().remove_ep()
                }
            },
            GateObject::MGate(g) => {
                if let Some(g) = g.upgrade() {
                    g.cgp.borrow_mut().remove_ep()
                }
            },
        }
    }
}

pub struct RGateObject {
    cgp: RefCell<CommonGateProperties>,
    loc: Cell<Option<(PEId, EpId)>>,
    addr: Cell<goff>,
    order: u32,
    msg_order: u32,
}

impl RGateObject {
    pub fn new(order: u32, msg_order: u32) -> Rc<Self> {
        Rc::new(RGateObject {
            cgp: RefCell::from(CommonGateProperties::new()),
            loc: Cell::from(None),
            addr: Cell::from(0),
            order,
            msg_order,
        })
    }

    pub fn cgp(&self) -> Ref<CommonGateProperties> {
        self.cgp.borrow()
    }

    pub fn cgp_mut(&self) -> RefMut<CommonGateProperties> {
        self.cgp.borrow_mut()
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

    pub fn adjust_rbuf(&self, base: goff) {
        self.addr.set(self.addr() + base);
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
    cgp: RefCell<CommonGateProperties>,
    rgate: Weak<RGateObject>,
    label: Label,
    credits: u32,
}

impl SGateObject {
    pub fn new(rgate: &Rc<RGateObject>, label: Label, credits: u32) -> Rc<Self> {
        Rc::new(SGateObject {
            cgp: RefCell::from(CommonGateProperties::new()),
            rgate: Rc::downgrade(rgate),
            label,
            credits,
        })
    }

    pub fn cgp(&self) -> Ref<CommonGateProperties> {
        self.cgp.borrow()
    }

    pub fn cgp_mut(&self) -> RefMut<CommonGateProperties> {
        self.cgp.borrow_mut()
    }

    pub fn rgate(&self) -> Rc<RGateObject> {
        self.rgate.upgrade().unwrap()
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
        self.rgate.upgrade().unwrap().print_loc(f)?;
        write!(f, ", lbl={:#x}, crd={}]", self.label, self.credits)
    }
}

pub struct MGateObject {
    cgp: RefCell<CommonGateProperties>,
    mem: mem::Allocation,
    perms: kif::Perm,
    derived: bool,
}

impl MGateObject {
    pub fn new(mem: mem::Allocation, perms: kif::Perm, derived: bool) -> Rc<Self> {
        Rc::new(MGateObject {
            cgp: RefCell::from(CommonGateProperties::new()),
            mem,
            perms,
            derived,
        })
    }

    pub fn cgp(&self) -> Ref<CommonGateProperties> {
        self.cgp.borrow()
    }

    pub fn cgp_mut(&self) -> RefMut<CommonGateProperties> {
        self.cgp.borrow_mut()
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
        Rc::new(ServObject {
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
    srv: Weak<ServObject>,
    creator: usize,
    ident: u64,
}

impl SessObject {
    pub fn new(srv: &Rc<ServObject>, creator: usize, ident: u64) -> Rc<Self> {
        Rc::new(SessObject {
            srv: Rc::downgrade(srv),
            creator,
            ident,
        })
    }

    pub fn service(&self) -> Rc<ServObject> {
        self.srv.upgrade().unwrap()
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
        Rc::new(SemObject {
            counter: Cell::from(counter),
            waiters: Cell::from(0),
        })
    }

    pub fn down(sem: &Rc<SemObject>) -> Result<(), Error> {
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
        Rc::new(PEObject {
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
    vpe: VPEId,
    ep: EpId,
    replies: u32,
    pe: Weak<PEObject>,
}

impl EPObject {
    pub fn new(is_std: bool, vpe: VPEId, ep: EpId, replies: u32, pe: &Rc<PEObject>) -> Rc<Self> {
        // TODO add to VPE
        Rc::new(EPObject {
            is_std,
            gate: RefCell::from(None),
            vpe,
            ep,
            replies,
            pe: Rc::downgrade(pe),
        })
    }

    pub fn pe_id(&self) -> PEId {
        self.pe.upgrade().unwrap().pe()
    }

    pub fn vpe(&self) -> VPEId {
        self.vpe
    }

    pub fn ep(&self) -> EpId {
        self.ep
    }

    pub fn replies(&self) -> u32 {
        self.replies
    }

    pub fn get_gate(&self) -> Ref<Option<GateObject>> {
        self.gate.borrow()
    }

    pub fn has_gate(&self) -> bool {
        self.gate.borrow().is_some()
    }

    pub fn set_gate(&self, g: GateObject) {
        self.gate.replace(Some(g));
    }

    pub fn remove_gate(&self) {
        self.gate.replace(None);
    }
}

impl Drop for EPObject {
    fn drop(&mut self) {
        // TODO remove from VPE

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
            self.vpe, self.ep, self.replies, self.pe
        )
    }
}
pub struct KMemObject {
    quota: usize,
    left: usize,
}

impl KMemObject {
    pub fn new(quota: usize) -> Rc<Self> {
        Rc::new(KMemObject { quota, left: quota })
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
}

impl MapObject {
    pub fn new(glob: GlobAddr, flags: kif::PageFlags) -> Rc<Self> {
        Rc::new(MapObject {
            glob: Cell::from(glob),
            flags: Cell::from(flags),
        })
    }

    pub fn global(&self) -> GlobAddr {
        self.glob.get()
    }

    pub fn flags(&self) -> kif::PageFlags {
        self.flags.get()
    }

    pub fn remap(
        &self,
        vpe: &Rc<VPE>,
        virt: goff,
        pages: usize,
        glob: GlobAddr,
        flags: kif::PageFlags,
    ) -> Result<(), Error> {
        self.map(vpe, virt, glob, pages, flags).and_then(|_| {
            self.glob.replace(glob);
            self.flags.replace(flags);
            Ok(())
        })
    }

    pub fn map(
        &self,
        vpe: &Rc<VPE>,
        virt: goff,
        glob: GlobAddr,
        pages: usize,
        flags: kif::PageFlags,
    ) -> Result<(), Error> {
        let pemux = pemng::get().pemux(vpe.pe_id());
        pemux.map(vpe.id(), virt, glob, pages, flags)
    }

    pub fn unmap(&self, vpe: &Rc<VPE>, virt: goff, pages: usize) {
        if vpe.has_app() {
            let pemux = pemng::get().pemux(vpe.pe_id());
            pemux
                .map(
                    vpe.id(),
                    virt,
                    self.global(),
                    pages,
                    kif::PageFlags::empty(),
                )
                .unwrap();
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
