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

use base::cell::RefCell;
use base::errors::{Code, Error};
use base::goff;
use base::kif;
use base::mem::GlobAddr;
use base::rc::Rc;
use base::tcu::{EpId, Label, PEId};
use core::fmt;
use core::ptr;
use thread;

use com::Service;
use mem;
use pes::{pemng, VPEId, VPE};

#[derive(Clone)]
pub enum KObject {
    RGate(Rc<RefCell<RGateObject>>),
    SGate(Rc<RefCell<SGateObject>>),
    MGate(Rc<RefCell<MGateObject>>),
    Map(Rc<RefCell<MapObject>>),
    Serv(Rc<RefCell<ServObject>>),
    Sess(Rc<RefCell<SessObject>>),
    Sem(Rc<RefCell<SemObject>>),
    VPE(Rc<RefCell<VPE>>),
    KMEM(Rc<RefCell<KMemObject>>),
    PE(Rc<RefCell<PEObject>>),
    EP(Rc<RefCell<EPObject>>),
}

impl fmt::Debug for KObject {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            KObject::SGate(ref s) => write!(f, "{:?}", s.borrow()),
            KObject::RGate(ref r) => write!(f, "{:?}", r.borrow()),
            KObject::MGate(ref m) => write!(f, "{:?}", m.borrow()),
            KObject::Map(ref m) => write!(f, "{:?}", m.borrow()),
            KObject::Serv(ref s) => write!(f, "{:?}", s.borrow()),
            KObject::Sess(ref s) => write!(f, "{:?}", s.borrow()),
            KObject::VPE(ref v) => write!(f, "{:?}", v.borrow()),
            KObject::Sem(ref s) => write!(f, "{:?}", s.borrow()),
            KObject::KMEM(ref k) => write!(f, "{:?}", k.borrow()),
            KObject::PE(ref p) => write!(f, "{:?}", p.borrow()),
            KObject::EP(ref e) => write!(f, "{:?}", e.borrow()),
        }
    }
}

pub struct RGateObject {
    loc: Option<(PEId, EpId)>,
    addr: goff,
    order: u32,
    msg_order: u32,
}

impl RGateObject {
    pub fn new(order: u32, msg_order: u32) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(RGateObject {
            loc: None,
            addr: 0,
            order,
            msg_order,
        }))
    }

    pub fn pe(&self) -> Option<PEId> {
        self.loc.map(|(pe, _)| pe)
    }

    pub fn ep(&self) -> Option<EpId> {
        self.loc.map(|(_, ep)| ep)
    }

    pub fn addr(&self) -> goff {
        self.addr
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
        self.addr != 0
    }

    pub fn activate(&mut self, pe: PEId, ep: EpId, addr: goff) {
        self.loc = Some((pe, ep));
        self.addr = addr;
    }

    pub fn adjust_rbuf(&mut self, base: goff) {
        self.addr += base;
    }

    pub fn deactivate(&mut self) {
        self.addr = 0;
        self.loc = None;
    }

    pub fn get_event(&self) -> thread::Event {
        self as *const Self as thread::Event
    }

    pub fn print_loc(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.loc {
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
            self.addr,
            self.size(),
            self.msg_size()
        )
    }
}

pub struct SGateObject {
    rgate: Rc<RefCell<RGateObject>>,
    label: Label,
    credits: u32,
}

impl SGateObject {
    pub fn new(rgate: &Rc<RefCell<RGateObject>>, label: Label, credits: u32) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(SGateObject {
            rgate: rgate.clone(),
            label,
            credits,
        }))
    }

    pub fn rgate(&self) -> &Rc<RefCell<RGateObject>> {
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
        self.rgate.borrow().print_loc(f)?;
        write!(f, ", lbl={:#x}, crd={}]", self.label, self.credits)
    }
}

pub struct MGateObject {
    mem: mem::Allocation,
    perms: kif::Perm,
    derived: bool,
}

impl MGateObject {
    pub fn new(mem: mem::Allocation, perms: kif::Perm, derived: bool) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(MGateObject {
            mem,
            perms,
            derived,
        }))
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
    serv: Rc<RefCell<Service>>,
    owner: bool,
    creator: usize,
}

impl ServObject {
    pub fn new(serv: Rc<RefCell<Service>>, owner: bool, creator: usize) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(ServObject {
            serv,
            owner,
            creator,
        }))
    }

    pub fn service(&self) -> &Rc<RefCell<Service>> {
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
            self.serv.borrow(),
            self.owner,
            self.creator
        )
    }
}

pub struct SessObject {
    srv: Rc<RefCell<ServObject>>,
    creator: usize,
    ident: u64,
}

impl SessObject {
    pub fn new(srv: &Rc<RefCell<ServObject>>, creator: usize, ident: u64) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(SessObject {
            srv: srv.clone(),
            creator,
            ident,
        }))
    }

    pub fn service(&self) -> &Rc<RefCell<ServObject>> {
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
            self.srv.borrow().service().borrow().name(),
            self.ident
        )
    }
}

pub struct SemObject {
    counter: u32,
    waiters: i32,
}

impl SemObject {
    pub fn new(counter: u32) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(SemObject {
            counter,
            waiters: 0,
        }))
    }

    pub fn down(sem: &Rc<RefCell<SemObject>>) -> Result<(), Error> {
        while unsafe { ptr::read_volatile(&sem.borrow().counter) } == 0 {
            sem.borrow_mut().waiters += 1;
            let event = sem.borrow().get_event();
            thread::ThreadManager::get().wait_for(event);
            if unsafe { ptr::read_volatile(&sem.borrow().waiters) } == -1 {
                return Err(Error::new(Code::RecvGone));
            }
            sem.borrow_mut().waiters -= 1;
        }
        sem.borrow_mut().counter -= 1;
        Ok(())
    }

    pub fn up(&mut self) {
        if self.waiters > 0 {
            thread::ThreadManager::get().notify(self.get_event(), None);
        }
        self.counter += 1;
    }

    pub fn revoke(&mut self) {
        if self.waiters > 0 {
            thread::ThreadManager::get().notify(self.get_event(), None);
        }
        self.waiters = -1;
    }

    fn get_event(&self) -> thread::Event {
        self as *const Self as thread::Event
    }
}

impl fmt::Debug for SemObject {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Sem[counter={}, waiters={}]", self.counter, self.waiters)
    }
}

pub struct PEObject {
    pe: PEId,
    eps: u32,
    vpes: u32,
}

impl PEObject {
    pub fn new(pe: PEId, eps: u32) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(PEObject { pe, eps, vpes: 0 }))
    }

    pub fn pe(&self) -> PEId {
        self.pe
    }

    pub fn eps(&self) -> u32 {
        self.eps
    }

    pub fn vpes(&self) -> u32 {
        self.vpes
    }

    pub fn has_quota(&self, eps: u32) -> bool {
        self.eps >= eps
    }

    pub fn alloc(&mut self, eps: u32) {
        klog!(
            PES,
            "PE[{}]: allocating {} EPs ({} total)",
            self.pe,
            eps,
            self.eps
        );
        assert!(self.eps >= eps);
        self.eps -= eps;
    }

    pub fn free(&mut self, eps: u32) {
        self.eps += eps;
        klog!(
            PES,
            "PE[{}]: freed {} EPs ({} total)",
            self.pe,
            eps,
            self.eps
        );
    }
}

impl fmt::Debug for PEObject {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "PE[id={}, eps={}, vpes={}]",
            self.pe, self.eps, self.vpes
        )
    }
}

pub struct EPObject {
    is_std: bool,
    vpe: VPEId,
    ep: EpId,
    replies: u32,
    pe: Rc<RefCell<PEObject>>,
}

impl EPObject {
    pub fn new(
        is_std: bool,
        vpe: VPEId,
        ep: EpId,
        replies: u32,
        pe: Rc<RefCell<PEObject>>,
    ) -> Rc<RefCell<Self>> {
        // TODO add to VPE
        Rc::new(RefCell::new(EPObject {
            is_std,
            vpe,
            ep,
            replies,
            pe,
        }))
    }

    pub fn pe_id(&self) -> PEId {
        self.pe.borrow().pe()
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
}

impl Drop for EPObject {
    fn drop(&mut self) {
        // TODO remove from VPE

        if !self.is_std {
            let pemux = pemng::get().pemux(self.pe.borrow().pe);
            pemux.free_eps(self.ep, 1 + self.replies);

            self.pe.borrow_mut().free(1 + self.replies);
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
    pub fn new(quota: usize) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(KMemObject { quota, left: quota }))
    }

    pub fn left(&self) -> usize {
        self.left
    }

    pub fn has_quota(&self, size: usize) -> bool {
        self.left >= size
    }

    pub fn alloc(&mut self, _size: usize) {
    }

    pub fn free(&mut self, _size: usize) {
    }
}

impl fmt::Debug for KMemObject {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "KMem[quota={:#x}, left={:#x}]", self.quota, self.left)
    }
}

pub struct MapObject {
    glob: GlobAddr,
    flags: kif::PageFlags,
}

impl MapObject {
    pub fn new(glob: GlobAddr, flags: kif::PageFlags) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(MapObject { glob, flags }))
    }

    pub fn global(&self) -> GlobAddr {
        self.glob
    }

    pub fn flags(&self) -> kif::PageFlags {
        self.flags
    }

    pub fn remap(
        &mut self,
        vpe: &VPE,
        virt: goff,
        pages: usize,
        glob: GlobAddr,
        flags: kif::PageFlags,
    ) -> Result<(), Error> {
        self.glob = glob;
        self.flags = flags;
        self.map(vpe, virt, pages)
    }

    pub fn map(&self, vpe: &VPE, virt: goff, pages: usize) -> Result<(), Error> {
        let pemux = pemng::get().pemux(vpe.pe_id());
        pemux.map(vpe.id(), virt, self.glob, pages, self.flags)
    }

    pub fn unmap(&self, vpe: &VPE, virt: goff, pages: usize) {
        if vpe.has_app() {
            let pemux = pemng::get().pemux(vpe.pe_id());
            pemux.map(vpe.id(), virt, self.glob, pages, kif::PageFlags::empty()).unwrap();
        }
    }
}

impl fmt::Debug for MapObject {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Map[glob={:?}, flags={:#x}]", self.glob, self.flags)
    }
}
