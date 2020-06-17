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

use base::cell::{RefMut, StaticCell};
use base::cfg;
use base::col::Treap;
use base::goff;
use base::kif::{CapRngDesc, CapSel, FIRST_FREE_SEL};
use base::rc::{Rc, Weak};
use core::cmp;
use core::fmt;
use core::ptr::{NonNull, Unique};

use cap::{GateEP, KObject};
use pes::{pemng, vpemng, VPE};

#[derive(Copy, Clone, PartialOrd, PartialEq, Eq)]
pub struct SelRange {
    start: CapSel,
    count: CapSel,
}

impl SelRange {
    pub fn new(sel: CapSel) -> Self {
        Self::new_range(sel, 1)
    }

    pub fn new_range(sel: CapSel, count: CapSel) -> Self {
        SelRange { start: sel, count }
    }
}

impl fmt::Debug for SelRange {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.start)
    }
}

impl cmp::Ord for SelRange {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        if self.start >= other.start && self.start < other.start + other.count {
            cmp::Ordering::Equal
        }
        else if self.start < other.start {
            cmp::Ordering::Less
        }
        else {
            cmp::Ordering::Greater
        }
    }
}

pub struct CapTable {
    caps: Treap<SelRange, Capability>,
    vpe: Weak<VPE>,
}

unsafe fn as_shared<T>(obj: &mut T) -> NonNull<T> {
    NonNull::from(Unique::new_unchecked(obj as *mut T))
}

impl CapTable {
    pub fn new() -> Self {
        CapTable {
            caps: Treap::new(),
            vpe: Weak::new(),
        }
    }

    pub fn set_vpe(&mut self, vpe: &Rc<VPE>) {
        self.vpe = Rc::downgrade(vpe);
    }

    pub fn unused(&self, sel: CapSel) -> bool {
        self.get(sel).is_none()
    }

    pub fn range_unused(&self, crd: &CapRngDesc) -> bool {
        for s in crd.start()..crd.start() + crd.count() {
            if self.get(s).is_some() {
                return false;
            }
        }
        return true;
    }

    pub fn get(&self, sel: CapSel) -> Option<&Capability> {
        self.caps.get(&SelRange::new(sel))
    }

    pub fn get_mut(&mut self, sel: CapSel) -> Option<&mut Capability> {
        self.caps.get_mut(&SelRange::new(sel))
    }

    pub fn insert(&mut self, mut cap: Capability) -> &mut Capability {
        unsafe {
            cap.table = Some(as_shared(self));
        }
        self.caps.insert(cap.sel_range().clone(), cap)
    }

    pub fn insert_as_child(&mut self, cap: Capability, parent_sel: CapSel) {
        unsafe {
            let parent: Option<NonNull<Capability>> = self.get_shared(parent_sel);
            self.do_insert(cap, parent);
        }
    }

    pub fn insert_as_child_from(
        &mut self,
        cap: Capability,
        mut par_tbl: RefMut<CapTable>,
        par_sel: CapSel,
    ) {
        unsafe {
            let parent = par_tbl.get_shared(par_sel);
            self.do_insert(cap, parent);
        }
    }

    unsafe fn get_shared(&mut self, sel: CapSel) -> Option<NonNull<Capability>> {
        self.caps
            .get_mut(&SelRange::new(sel))
            .map(|cap| NonNull::new_unchecked(cap))
    }

    unsafe fn do_insert(&mut self, child: Capability, parent: Option<NonNull<Capability>>) {
        let mut child_cap = self.insert(child);
        if let Some(parent_cap) = parent {
            (*parent_cap.as_ptr()).inherit(&mut child_cap);
        }
    }

    pub fn obtain(&mut self, sel: CapSel, cap: &mut Capability, child: bool) {
        let mut nc: Capability = (*cap).clone();
        nc.sels = SelRange::new(sel);
        if child {
            cap.inherit(self.insert(nc));
        }
        else {
            self.insert(nc).inherit(cap);
        }
    }

    pub fn revoke(&mut self, crd: CapRngDesc, own: bool) {
        for sel in crd.start()..crd.start() + crd.count() {
            self.get_mut(sel).map(|cap| {
                if own {
                    cap.revoke(false, false);
                }
                else {
                    unsafe {
                        cap.child.map(|child| (*child.as_ptr()).revoke(true, true));
                    }
                }
            });
        }
    }

    pub fn revoke_all(&mut self, all: bool) {
        let mut tmp = Treap::new();

        while let Some(cap) = self.caps.get_root_mut() {
            if all || cap.sel() >= FIRST_FREE_SEL {
                // on revoke_all, we consider all revokes foreign to notify about invalidate send gates
                // in any case. on explicit revokes, we only do that if it's a derived cap.
                cap.revoke(false, true);
            }
            else {
                // remove from tree and insert them later
                let sels = *cap.sel_range();
                let cap = self.caps.remove(&sels).unwrap();
                tmp.insert(cap.sel(), cap);
            }
        }

        // insert them again
        while let Some(cap) = tmp.remove_root() {
            self.caps.insert(*cap.sel_range(), cap);
        }
    }
}

impl fmt::Debug for CapTable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "CapTable[\n{:?}]", self.caps)
    }
}

#[derive(Clone)]
pub struct Capability {
    sels: SelRange,
    obj: KObject,
    table: Option<NonNull<CapTable>>,
    child: Option<NonNull<Capability>>,
    parent: Option<NonNull<Capability>>,
    next: Option<NonNull<Capability>>,
    prev: Option<NonNull<Capability>>,
}

impl Capability {
    pub fn new(sel: CapSel, obj: KObject) -> Self {
        Self::new_range(SelRange::new(sel), obj)
    }

    pub fn new_range(sels: SelRange, obj: KObject) -> Self {
        Capability {
            sels,
            obj,
            table: None,
            child: None,
            parent: None,
            next: None,
            prev: None,
        }
    }

    pub fn sel_range(&self) -> &SelRange {
        &self.sels
    }

    pub fn sel(&self) -> CapSel {
        self.sels.start
    }

    pub fn len(&self) -> CapSel {
        self.sels.count
    }

    pub fn get(&self) -> &KObject {
        &self.obj
    }

    pub fn get_mut(&mut self) -> &mut KObject {
        &mut self.obj
    }

    pub fn has_parent(&self) -> bool {
        self.parent.is_some()
    }

    pub fn inherit(&mut self, child: &mut Capability) {
        unsafe {
            child.parent = Some(as_shared(self));
            child.child = None;
            child.next = self.child;
            child.prev = None;
            if let Some(n) = child.next {
                (*n.as_ptr()).prev = Some(as_shared(child));
            }
            self.child = Some(as_shared(child));
        }
    }

    pub fn get_root(&mut self) -> &mut Capability {
        if let Some(mut cap) = self.parent {
            unsafe {
                while let Some(p) = (*cap.as_ptr()).parent {
                    cap = p;
                }
                &mut *cap.as_ptr()
            }
        }
        else {
            self
        }
    }

    pub fn find_child<P>(&mut self, pred: P) -> Option<&mut Capability>
    where
        P: Fn(&Capability) -> bool,
    {
        let mut next = self.child;
        while let Some(n) = next {
            unsafe {
                if pred(&*n.as_ptr()) {
                    return Some(&mut *n.as_ptr());
                }
                next = (*n.as_ptr()).next;
            }
        }
        None
    }

    fn revoke(&mut self, rev_next: bool, foreign: bool) {
        unsafe {
            if let Some(n) = self.next {
                (*n.as_ptr()).prev = self.prev;
            }
            if let Some(p) = self.prev {
                (*p.as_ptr()).next = self.next;
            }
            if let Some(p) = self.parent {
                if self.prev.is_none() {
                    let child = &mut (*p.as_ptr()).child;
                    *child = self.next;
                }
            }
            self.revoke_rec(rev_next, foreign);
        }
    }

    fn revoke_rec(&mut self, rev_next: bool, foreign: bool) {
        self.release(foreign);

        unsafe {
            // remove it from the table
            let sels = SelRange::new(self.sel());
            let cap = self.table_mut().caps.remove(&sels).unwrap();

            if let Some(c) = cap.child {
                (*c.as_ptr()).revoke_rec(true, true);
            }
            // on the first level, we don't want to revoke siblings
            if rev_next {
                if let Some(n) = cap.next {
                    (*n.as_ptr()).revoke_rec(true, true);
                }
            }
        }
    }

    fn table(&self) -> &CapTable {
        unsafe { &*self.table.unwrap().as_ptr() }
    }

    fn table_mut(&mut self) -> &mut CapTable {
        unsafe { &mut *self.table.unwrap().as_ptr() }
    }

    fn vpe(&self) -> Option<Rc<VPE>> {
        self.table().vpe.upgrade()
    }

    fn invalidate_ep(mut cgp: RefMut<GateEP>, foreign: bool) {
        if let Some(ep) = cgp.get_ep() {
            let pemux = pemng::get().pemux(ep.pe_id());
            let vpe_id = ep.vpe().id();
            // if that fails, just ignore it
            pemux.invalidate_ep(vpe_id, ep.ep(), true, true).ok();

            // notify PEMux about the invalidation if it's not a self-invalidation (technically,
            // <foreign> indicates whether we're in the first level of revoke, but since it is just a
            // notification, we can ignore the case that someone delegated a cap to itself).
            if foreign {
                pemux.notify_invalidate(vpe_id, ep.ep()).ok();
            }

            cgp.remove_ep();
        }
    }

    fn release(&mut self, foreign: bool) {
        match self.obj {
            KObject::VPE(ref v) => {
                // remove VPE if we revoked the root capability
                if let Some(v) = v.upgrade() {
                    if self.parent.is_none() && !v.is_root() {
                        let id = v.id();
                        vpemng::get().remove_vpe(id);
                    }
                }
            },

            KObject::SGate(ref mut o) => {
                Self::invalidate_ep(o.gate_ep_mut(), foreign);
            },

            KObject::RGate(ref mut o) => {
                Self::invalidate_ep(o.gate_ep_mut(), false);
            },

            KObject::MGate(ref mut o) => {
                Self::invalidate_ep(o.gate_ep_mut(), false);
            },

            KObject::Serv(ref s) => {
                s.service().abort();
            },

            KObject::Sess(ref _s) => {
                // TODO if this is the root session, drop messages at server
            },

            KObject::Map(ref m) => {
                if m.mapped() {
                    let virt = (self.sel() as goff) << cfg::PAGE_BITS;
                    // TODO currently, it can happen that we've already stopped the VPE, but still
                    // accept/continue a syscall that inserts something into the VPE's table. So,
                    // be careful here that the VPE can be None.
                    if let Some(vpe) = self.vpe() {
                        m.unmap(&vpe, virt, self.len() as usize);
                    }
                }
            },

            KObject::Sem(ref s) => {
                s.revoke();
            },

            _ => {},
        }
    }
}

fn print_childs(cap: NonNull<Capability>, f: &mut fmt::Formatter) -> fmt::Result {
    static LAYER: StaticCell<u32> = StaticCell::new(5);
    use core::fmt::Write;
    let mut next = Some(cap);
    loop {
        match next {
            None => return Ok(()),
            Some(n) => unsafe {
                f.write_char('\n')?;
                for _ in 0..*LAYER {
                    f.write_char(' ')?;
                }
                LAYER.set(*LAYER + 1);
                write!(f, "=> {:?}", *n.as_ptr())?;
                LAYER.set(*LAYER - 1);

                next = (*n.as_ptr()).next;
            },
        }
    }
}

impl fmt::Debug for Capability {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Cap[vpe={}, sel={}, len={}, obj={:?}]",
            self.vpe().unwrap().id(),
            self.sel(),
            self.len(),
            self.obj
        )?;
        if let Some(c) = self.child {
            print_childs(c, f)?;
        }
        Ok(())
    }
}
