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

use base::cell::{Cell, Ref, RefCell, RefMut, StaticCell};
use base::errors::{Code, Error};
use base::goff;
use base::kif;
use base::mem::GlobAddr;
use base::rc::{Rc, SRc, Weak};
use base::tcu::{EpId, Label, PEId};
use base::util;
use core::fmt;
use core::ptr;

use crate::com::Service;
use crate::mem;
use crate::pes::{PEMng, State, VPE};

#[derive(Clone)]
pub enum KObject {
    RGate(SRc<RGateObject>),
    SGate(SRc<SGateObject>),
    MGate(SRc<MGateObject>),
    Map(SRc<MapObject>),
    Serv(SRc<ServObject>),
    Sess(SRc<SessObject>),
    Sem(SRc<SemObject>),
    // Only VPEManager owns a VPE (Rc<VPE>). Break cycle here by using Weak
    VPE(Weak<VPE>),
    KMem(SRc<KMemObject>),
    PE(SRc<PEObject>),
    EP(Rc<EPObject>),
}

const fn kobj_size<T>() -> usize {
    let size = util::size_of::<T>();
    if size <= 64 {
        64 + crate::slab::HEADER_SIZE
    }
    else if size <= 128 {
        128 + crate::slab::HEADER_SIZE
    }
    else {
        size + util::size_of::<base::mem::heap::HeapArea>()
    }
}

static KOBJ_SIZES: [usize; 11] = [
    kobj_size::<SGateObject>(),
    kobj_size::<RGateObject>(),
    kobj_size::<MGateObject>(),
    kobj_size::<MapObject>(),
    kobj_size::<ServObject>(),
    kobj_size::<SessObject>(),
    kobj_size::<VPE>(),
    kobj_size::<SemObject>(),
    kobj_size::<KMemObject>(),
    kobj_size::<PEObject>(),
    kobj_size::<EPObject>(),
];

impl KObject {
    pub fn size(&self) -> usize {
        // get the index in the enum
        let idx: usize = unsafe { *(self as *const _ as *const usize) };
        KOBJ_SIZES[idx]
    }
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
            KObject::KMem(k) => write!(f, "{:?}", k),
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
    RGate(SRc<RGateObject>),
    SGate(SRc<SGateObject>),
    MGate(SRc<MGateObject>),
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
    pub fn new(order: u32, msg_order: u32) -> SRc<Self> {
        SRc::new(Self {
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

    pub fn location(&self) -> Option<(PEId, EpId)> {
        self.loc.get()
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
    rgate: SRc<RGateObject>,
    label: Label,
    credits: u32,
}

impl SGateObject {
    pub fn new(rgate: &SRc<RGateObject>, label: Label, credits: u32) -> SRc<Self> {
        SRc::new(Self {
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

    pub fn rgate(&self) -> &SRc<RGateObject> {
        &self.rgate
    }

    pub fn label(&self) -> Label {
        self.label
    }

    pub fn credits(&self) -> u32 {
        self.credits
    }

    pub fn invalidate_reply_eps(&self) {
        // is the send gate activated?
        if let Some(sep) = self.gate_ep().get_ep() {
            // is the associated receive gate activated?
            if let Some((recv_pe, recv_ep)) = self.rgate().location() {
                let pemux = PEMng::get().pemux(sep.pe_id());
                pemux
                    .invalidate_reply_eps(recv_pe, recv_ep, sep.ep())
                    .unwrap();
            }
        }
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
    pub fn new(mem: mem::Allocation, perms: kif::Perm, derived: bool) -> SRc<Self> {
        SRc::new(Self {
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
    serv: SRc<Service>,
    owner: bool,
    creator: usize,
}

impl ServObject {
    pub fn new(serv: SRc<Service>, owner: bool, creator: usize) -> SRc<Self> {
        SRc::new(Self {
            serv,
            owner,
            creator,
        })
    }

    pub fn service(&self) -> &SRc<Service> {
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
    srv: SRc<ServObject>,
    creator: usize,
    ident: u64,
}

impl SessObject {
    pub fn new(srv: &SRc<ServObject>, creator: usize, ident: u64) -> SRc<Self> {
        SRc::new(Self {
            srv: srv.clone(),
            creator,
            ident,
        })
    }

    pub fn service(&self) -> &SRc<ServObject> {
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
    pub fn new(counter: u32) -> SRc<Self> {
        SRc::new(Self {
            counter: Cell::from(counter),
            waiters: Cell::from(0),
        })
    }

    pub fn down_async(sem: &SRc<Self>) -> Result<(), Error> {
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
    total_eps: u32,
    cur_eps: Cell<u32>,
    cur_vpes: Cell<u32>,
}

impl PEObject {
    pub fn new(pe: PEId, eps: u32) -> SRc<Self> {
        SRc::new(Self {
            pe,
            total_eps: eps,
            cur_eps: Cell::from(eps),
            cur_vpes: Cell::from(0),
        })
    }

    pub fn pe(&self) -> PEId {
        self.pe
    }

    pub fn eps(&self) -> u32 {
        self.cur_eps.get()
    }

    pub fn vpes(&self) -> u32 {
        self.cur_vpes.get()
    }

    pub fn has_quota(&self, eps: u32) -> bool {
        self.eps() >= eps
    }

    pub fn add_vpe(&self) {
        self.cur_vpes.set(self.vpes() + 1);
    }

    pub fn rem_vpe(&self) {
        assert!(self.vpes() > 0);
        self.cur_vpes.set(self.vpes() - 1);
    }

    pub fn alloc(&self, eps: u32) {
        klog!(
            PES,
            "PE[{}]: allocating {} EPs ({} total)",
            self.pe,
            eps,
            self.eps()
        );
        assert!(self.eps() >= eps);
        self.cur_eps.set(self.eps() - eps);
    }

    pub fn free(&self, eps: u32) {
        assert!(self.eps() + eps <= self.total_eps);
        self.cur_eps.set(self.eps() + eps);
        klog!(
            PES,
            "PE[{}]: freed {} EPs ({} total)",
            self.pe,
            eps,
            self.eps()
        );
    }

    pub fn revoke(&self, parent: &PEObject) {
        // grant the EPs back to our parent
        parent.free(self.eps());
        assert!(self.eps() == self.total_eps);
    }
}

impl fmt::Debug for PEObject {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "PE[id={}, eps={}, vpes={}]",
            self.pe,
            self.eps(),
            self.vpes()
        )
    }
}

pub struct EPObject {
    is_std: bool,
    gate: RefCell<Option<GateObject>>,
    vpe: Weak<VPE>,
    ep: EpId,
    replies: u32,
    pe: SRc<PEObject>,
}

impl EPObject {
    pub fn new(
        is_std: bool,
        vpe: Weak<VPE>,
        ep: EpId,
        replies: u32,
        pe: &SRc<PEObject>,
    ) -> Rc<Self> {
        let maybe_vpe = vpe.upgrade();
        let ep = Rc::new(Self {
            is_std,
            gate: RefCell::from(None),
            vpe,
            ep,
            replies,
            pe: pe.clone(),
        });
        if let Some(v) = maybe_vpe {
            v.add_ep(ep.clone());
        }
        ep
    }

    pub fn pe_id(&self) -> PEId {
        self.pe.pe()
    }

    pub fn vpe(&self) -> Option<Rc<VPE>> {
        self.vpe.upgrade()
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
            let pemux = PEMng::get().pemux(pe_id);

            // invalidate receive and send EPs
            match gate {
                GateObject::RGate(_) | GateObject::SGate(_) => {
                    pemux.invalidate_ep(self.vpe().unwrap().id(), self.ep, force, false)?;
                    invalidated = true;
                },
                _ => {},
            }

            match gate {
                // invalidate reply EPs
                GateObject::SGate(s) => s.invalidate_reply_eps(),
                // deactivate receive gate
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
            let pemux = PEMng::get().pemux(self.pe.pe);
            pemux.free_eps(self.ep, 1 + self.replies);

            self.pe.free(1 + self.replies);
        }
    }
}

impl fmt::Debug for EPObject {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "EP[vpe={}, ep={}, replies={}, pe={:?}]",
            self.vpe().unwrap().id(),
            self.ep,
            self.replies,
            self.pe
        )
    }
}

static NEXT_KMEM_ID: StaticCell<usize> = StaticCell::new(0);

pub struct KMemObject {
    id: usize,
    quota: usize,
    left: Cell<usize>,
}

impl KMemObject {
    pub fn new(quota: usize) -> SRc<Self> {
        let id = *NEXT_KMEM_ID;
        *NEXT_KMEM_ID.get_mut() += 1;

        let kmem = SRc::new(Self {
            id,
            quota,
            left: Cell::from(quota),
        });
        klog!(KMEM, "{:?} created", kmem);
        kmem
    }

    pub fn quota(&self) -> usize {
        self.quota
    }

    pub fn left(&self) -> usize {
        self.left.get()
    }

    pub fn has_quota(&self, size: usize) -> bool {
        self.left.get() >= size
    }

    pub fn alloc(&self, vpe: &VPE, sel: kif::CapSel, size: usize) -> bool {
        klog!(
            KMEM,
            "{:?} VPE{}:{} allocates {}b (sel={})",
            self,
            vpe.id(),
            vpe.name(),
            size,
            sel,
        );

        if self.has_quota(size) {
            self.left.set(self.left() - size);
            true
        }
        else {
            false
        }
    }

    pub fn free(&self, vpe: &VPE, sel: kif::CapSel, size: usize) {
        assert!(self.left() + size <= self.quota);
        self.left.set(self.left() + size);

        klog!(
            KMEM,
            "{:?} VPE{}:{} freed {}b (sel={})",
            self,
            vpe.id(),
            vpe.name(),
            size,
            sel
        );
    }

    pub fn revoke(&self, vpe: &VPE, sel: kif::CapSel, parent: &KMemObject) {
        // grant the kernel memory back to our parent
        parent.free(vpe, sel, self.left());
        assert!(self.left() == self.quota);
    }
}

impl fmt::Debug for KMemObject {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "KMem[id={}, quota={}, left={}]",
            self.id,
            self.quota,
            self.left()
        )
    }
}

impl Drop for KMemObject {
    fn drop(&mut self) {
        klog!(KMEM, "{:?} dropped", self);
        assert!(self.left() == self.quota);
    }
}

pub struct MapObject {
    glob: Cell<GlobAddr>,
    flags: Cell<kif::PageFlags>,
    mapped: Cell<bool>,
}

impl MapObject {
    pub fn new(glob: GlobAddr, flags: kif::PageFlags) -> SRc<Self> {
        SRc::new(Self {
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

    pub fn map_async(
        &self,
        vpe: &VPE,
        virt: goff,
        glob: GlobAddr,
        pages: usize,
        flags: kif::PageFlags,
    ) -> Result<(), Error> {
        let pemux = PEMng::get().pemux(vpe.pe_id());
        pemux
            .map_async(vpe.id(), virt, glob, pages, flags)
            .map(|_| {
                self.glob.replace(glob);
                self.flags.replace(flags);
                self.mapped.set(true);
            })
    }

    pub fn unmap_async(&self, vpe: &VPE, virt: goff, pages: usize) {
        // TODO currently, it can happen that we've already stopped the VPE, but still
        // accept/continue a syscall that inserts something into the VPE's table.
        if vpe.state() != State::DEAD {
            let pemux = PEMng::get().pemux(vpe.pe_id());
            pemux.unmap_async(vpe.id(), virt, pages).ok();
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
