/*
 * Copyright (C) 2020-2022 Nils Asmussen, Barkhausen Institut
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

use base::cell::{RefCell, RefMut, StaticCell};
use base::cfg;
use base::col::Treap;
use base::errors::{Code, Error};
use base::goff;
use base::io::LogFlags;
use base::kif::{CapRngDesc, CapSel, SEL_ACT, SEL_KMEM, SEL_TILE};
use base::log;
use base::mem::size_of;
use base::rc::Rc;
use core::cmp;
use core::fmt;
use core::ptr::NonNull;

use crate::cap::{EPObject, GateEP, KObject};
use crate::ktcu;
use crate::tiles::{tilemng, Activity, ActivityMng};

#[derive(Copy, Clone, PartialEq, Eq)]
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.start)
    }
}

impl cmp::PartialOrd for SelRange {
    fn partial_cmp(&self, other: &SelRange) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
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
    act: Option<NonNull<Activity>>,
}

unsafe fn as_shared<T>(obj: &mut T) -> NonNull<T> {
    NonNull::new_unchecked(obj as *mut T)
}

impl Default for CapTable {
    fn default() -> Self {
        Self {
            caps: Treap::new(),
            act: None,
        }
    }
}

impl CapTable {
    fn activity(&self) -> &Activity {
        unsafe { &(*self.act.unwrap().as_ptr()) }
    }

    pub fn set_activity(&mut self, act: &Rc<Activity>) {
        let act_ptr = unsafe { NonNull::new_unchecked(Rc::as_ptr(act) as *mut _) };
        self.act = Some(act_ptr);
    }

    pub fn is_empty(&self) -> bool {
        self.caps.is_empty()
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
        true
    }

    pub fn get(&self, sel: CapSel) -> Option<&Capability> {
        self.caps.get(&SelRange::new(sel))
    }

    pub fn get_mut(&mut self, sel: CapSel) -> Option<&mut Capability> {
        self.caps.get_mut(&SelRange::new(sel))
    }

    #[inline(always)]
    pub fn insert(&mut self, cap: Capability) -> Result<(), Error> {
        self.insert_new(cap, None)
    }

    #[inline(always)]
    pub fn insert_as_child(&mut self, cap: Capability, parent_sel: CapSel) -> Result<(), Error> {
        unsafe {
            let parent = self.get_shared(parent_sel);
            self.insert_new(cap, parent)
        }
    }

    #[inline(always)]
    pub fn insert_as_child_from(
        &mut self,
        cap: Capability,
        mut par_tbl: RefMut<'_, CapTable>,
        par_sel: CapSel,
    ) -> Result<(), Error> {
        unsafe {
            let parent = par_tbl.get_shared(par_sel);
            self.insert_new(cap, parent)
        }
    }

    #[inline(always)]
    unsafe fn get_shared(&mut self, sel: CapSel) -> Option<NonNull<Capability>> {
        self.caps
            .get_mut(&SelRange::new(sel))
            .map(|cap| NonNull::new_unchecked(cap))
    }

    #[inline(always)]
    fn insert_new(
        &mut self,
        cap: Capability,
        parent: Option<NonNull<Capability>>,
    ) -> Result<(), Error> {
        let act = self.activity();
        if !act
            .kmem()
            .alloc(act, cap.sel(), cap.obj.size() + Capability::size())
        {
            return Err(Error::new(Code::NoSpace));
        }

        unsafe {
            let child_cap = self.do_insert(cap);
            if let Some(parent) = parent {
                (*parent.as_ptr()).inherit(child_cap);
            }
            log!(LogFlags::KernCaps, "Creating cap {:?}", child_cap);
        }
        Ok(())
    }

    pub fn obtain(&mut self, sel: CapSel, cap: &mut Capability, child: bool) -> Result<(), Error> {
        let act = self.activity();
        if !act.kmem().alloc(act, sel, Capability::size()) {
            return Err(Error::new(Code::NoSpace));
        }

        let mut nc: Capability = (*cap).clone();
        nc.sels = SelRange::new(sel);
        nc.derived = true;

        let nc = self.do_insert(nc);
        log!(LogFlags::KernCaps, "Cloning cap {:?}", nc);
        if child {
            cap.inherit(nc);
        }
        else {
            nc.inherit(cap);
        }
        Ok(())
    }

    fn do_insert(&mut self, mut cap: Capability) -> &mut Capability {
        unsafe {
            cap.table = Some(as_shared(self));
        }
        self.caps.insert(*cap.sel_range(), cap)
    }

    pub fn revoke_async(tbl: &RefCell<Self>, crd: CapRngDesc, own: bool) -> Result<(), Error> {
        let mut sel = crd.start();
        while sel < crd.start() + crd.count() {
            let tbl_ref = tbl.borrow_mut();
            match RefMut::filter_map(tbl_ref, |t| t.get_mut(sel)) {
                Ok(cap) => {
                    if !cap.can_revoke() {
                        return Err(Error::new(Code::NotRevocable));
                    }

                    let len = cap.len();
                    if own {
                        Capability::revoke_async(cap, false, false);
                    }
                    else if let Ok(child) = RefMut::filter_map(cap, |c| c.child.as_mut()) {
                        unsafe {
                            Capability::revoke_async(
                                RefMut::map(child, |c| &mut (*c.as_ptr())),
                                true,
                                true,
                            );
                        }
                    }
                    sel += len;
                },

                Err(_tbl) => {
                    sel += 1;
                },
            }
        }
        Ok(())
    }

    pub fn revoke_all_async(tbl: &RefCell<Self>) {
        loop {
            let tbl_ref = tbl.borrow_mut();
            match RefMut::filter_map(tbl_ref, |t| t.caps.get_root_mut()) {
                Ok(cap) => {
                    // on revoke_all, we consider all revokes foreign to notify about invalidate send gates
                    // in any case. on explicit revokes, we only do that if it's a derived cap.
                    Capability::revoke_async(cap, false, true)
                },
                Err(_tbl) => break,
            }
        }
    }
}

impl fmt::Debug for CapTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
    derived: bool,
}

impl Capability {
    const fn size() -> usize {
        base::const_assert!(size_of::<Capability>() <= 128);
        128 + crate::slab::HEADER_SIZE
    }

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
            derived: false,
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

    pub fn has_parent(&self) -> bool {
        self.parent.is_some()
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

    fn inherit(&mut self, child: &mut Capability) {
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

    fn revoke_async(mut cap: RefMut<'_, Self>, rev_next: bool, foreign: bool) {
        unsafe {
            if let Some(n) = cap.next {
                (*n.as_ptr()).prev = cap.prev;
            }
            if let Some(p) = cap.prev {
                (*p.as_ptr()).next = cap.next;
            }
            if let Some(p) = cap.parent {
                if cap.prev.is_none() {
                    let child = &mut (*p.as_ptr()).child;
                    *child = cap.next;
                }
            }
        }

        // remove it from the table
        let sels = SelRange::new(cap.sel());
        let owned_cap = cap.table_mut().caps.remove(&sels).unwrap();
        drop(cap);

        owned_cap.revoke_rec_async(rev_next, foreign);
    }

    fn revoke_rec_async(self, rev_next: bool, foreign: bool) {
        unsafe {
            if let Some(c) = self.child {
                let child_cap = &mut (*c.as_ptr());
                let child_sels = SelRange::new(child_cap.sel());
                let owned_child = child_cap.table_mut().caps.remove(&child_sels).unwrap();
                owned_child.revoke_rec_async(true, true);
            }
            // on the first level, we don't want to revoke siblings
            if rev_next {
                let mut cap_next = self.next;
                while let Some(n) = cap_next {
                    let sib = &mut *n.as_ptr();
                    // remove it from the table
                    let sels = SelRange::new(sib.sel());
                    let owned_sib = sib.table_mut().caps.remove(&sels).unwrap();

                    if let Some(c) = owned_sib.child {
                        let child_cap = &mut (*c.as_ptr());
                        let child_sels = SelRange::new(child_cap.sel());
                        let owned_child = child_cap.table_mut().caps.remove(&child_sels).unwrap();
                        owned_child.revoke_rec_async(true, true);
                    }

                    cap_next = owned_sib.next;
                    owned_sib.release_async(true);
                }
            }
        }

        // do that after making the cap inaccessible to make sure that no one can still access it,
        // because we might do a thread switch in release().
        self.release_async(foreign);
    }

    fn table(&self) -> &CapTable {
        unsafe { &*self.table.unwrap().as_ptr() }
    }

    fn table_mut(&mut self) -> &mut CapTable {
        unsafe { &mut *self.table.unwrap().as_ptr() }
    }

    fn activity(&self) -> &Activity {
        self.table().activity()
    }

    fn invalidate_ep(mut cgp: RefMut<'_, GateEP>, foreign: bool) {
        if let Some(ep) = cgp.get_ep() {
            let mut tilemux = tilemng::tilemux(ep.tile_id());
            if let Some(act) = ep.activity() {
                // if that fails, just ignore it
                tilemux
                    .invalidate_ep(act.id(), ep.ep(), !ep.is_rgate(), true)
                    .ok();

                // notify TileMux about the invalidation if it's not a self-invalidation (technically,
                // `foreign` indicates whether we're in the first level of revoke, but since it is
                // just a notification, we can ignore the case that someone delegated a cap to
                // itself).
                if foreign {
                    tilemux.notify_invalidate(act.id(), ep.ep()).ok();
                }
            }
            else {
                // force invalidate without activity (no notification etc.)
                ktcu::invalidate_ep_remote(ep.tile_id(), ep.ep(), true).ok();
            }

            EPObject::revoke(&ep);

            cgp.remove_ep();
        }
    }

    fn can_revoke(&self) -> bool {
        match self.obj {
            KObject::KMem(ref k) => k.left() == k.quota(),
            KObject::Tile(ref tile) => tile.activities() == 0,
            _ => true,
        }
    }

    fn release_async(mut self, foreign: bool) {
        log!(LogFlags::KernCaps, "Freeing cap {:?}", self);

        let act = self.activity();
        let sel = self.sel();
        if !self.derived {
            // if it's not derived, we created the cap and thus will also free the kobject
            act.kmem()
                .free(act, sel, Capability::size() + self.obj.size());
        }
        else {
            // give quota for cap back in every case
            act.kmem().free(act, sel, Capability::size());
        }

        match self.obj {
            KObject::Activity(ref v) => {
                // remove activity if we revoked the root capability and if it's not the own activity
                if let Some(v) = v.upgrade() {
                    if sel != SEL_ACT && self.parent.is_none() && !v.is_root() {
                        ActivityMng::remove_activity_async(v.id());
                    }
                }
            },

            KObject::EP(ref mut e) => {
                EPObject::revoke(e);
            },

            KObject::Tile(ref mut tile) => {
                // if the cap is derived, it doesn't own the kobj. if it's the activity's own Tile, the
                // kobj always belongs to the parent (but derived is false).
                if !self.derived && sel != SEL_TILE {
                    if let Some(parent) = self.parent {
                        let parent = unsafe { &(*parent.as_ptr()) };
                        if let KObject::Tile(p) = parent.get() {
                            tile.revoke_async(p);
                        }
                    }
                }
            },

            KObject::KMem(ref k) => {
                // see above
                if !self.derived && sel != SEL_KMEM {
                    if let Some(parent) = self.parent {
                        let parent = unsafe { &(*parent.as_ptr()) };
                        if let KObject::KMem(p) = parent.get() {
                            k.revoke(parent.activity(), parent.sel(), p);
                        }
                    }
                }
            },

            KObject::SGate(ref mut o) => {
                o.invalidate_reply_eps();
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
                    m.unmap_async(act, virt, self.len() as usize);
                }
            },

            KObject::Sem(ref s) => {
                s.revoke();
            },
        }
    }
}

fn print_childs(cap: NonNull<Capability>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    static LAYER: StaticCell<u32> = StaticCell::new(5);
    use core::fmt::Write;
    let mut next = Some(cap);
    loop {
        match next {
            None => return Ok(()),
            Some(n) => unsafe {
                f.write_char('\n')?;
                for _ in 0..LAYER.get() {
                    f.write_char(' ')?;
                }
                LAYER.set(LAYER.get() + 1);
                write!(f, "=> {:?}", *n.as_ptr())?;
                LAYER.set(LAYER.get() - 1);

                next = (*n.as_ptr()).next;
            },
        }
    }
}

impl fmt::Debug for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Cap[act={}, sel={}, len={}, obj={:?}]",
            self.activity().id(),
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
