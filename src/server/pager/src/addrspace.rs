/*
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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
use m3::cfg;
use m3::col::Vec;
use m3::com::{GateIStream, RecvGate, SGateArgs, SendGate};
use m3::errors::{Code, Error};
use m3::goff;
use m3::kif::{PageFlags, Perm};
use m3::log;
use m3::reply_vmsg;
use m3::serialize::M3Deserializer;
use m3::server::SessId;
use m3::session::{MapFlags, ServerSession};
use m3::tcu::Label;
use m3::tiles::Activity;
use m3::util::math;
use resmng::childs;

use crate::dataspace::DataSpace;

const MAX_VIRT_ADDR: goff = cfg::MEM_CAP_END as goff - 1;

pub struct AddrSpace {
    crt: usize,
    parent: Option<SessId>,
    sess: ServerSession,
    child: Option<childs::Id>,
    owner: Option<Selector>,
    sgates: Vec<SendGate>,
    ds: Vec<DataSpace>,
}

impl AddrSpace {
    pub fn new(
        crt: usize,
        sess: ServerSession,
        parent: Option<SessId>,
        child: Option<childs::Id>,
    ) -> Self {
        AddrSpace {
            crt,
            parent,
            sess,
            child,
            owner: None,
            sgates: Vec::new(),
            ds: Vec::new(),
        }
    }

    pub fn creator(&self) -> usize {
        self.crt
    }

    pub fn id(&self) -> SessId {
        self.sess.ident() as SessId
    }

    pub fn child_id(&self) -> Option<childs::Id> {
        self.child
    }

    pub fn parent(&self) -> Option<SessId> {
        self.parent
    }

    pub fn has_owner(&self) -> bool {
        self.owner.is_some()
    }

    pub fn init(
        &mut self,
        child: Option<childs::Id>,
        act: Option<Selector>,
    ) -> Result<Selector, Error> {
        if self.owner.is_some() {
            Err(Error::new(Code::InvArgs))
        }
        else {
            let act = act.unwrap_or_else(|| Activity::own().alloc_sel());
            log!(
                crate::LOG_DEF,
                "[{}] pager::init(child={:?}, act={})",
                self.id(),
                child,
                act
            );
            if let Some(c) = child {
                assert!(self.child.is_none());
                self.child = Some(c);
            }
            else {
                assert!(self.child.is_some());
            }
            self.owner = Some(act);
            Ok(act)
        }
    }

    pub fn add_sgate(&mut self, rgate: &RecvGate) -> Result<Selector, Error> {
        log!(crate::LOG_DEF, "[{}] pager::add_sgate()", self.id());

        let sgate = SendGate::new_with(SGateArgs::new(rgate).label(self.id() as Label).credits(1))?;
        let sel = sgate.sel();
        self.sgates.push(sgate);

        Ok(sel)
    }

    pub fn clone(&mut self, is: &mut GateIStream<'_>, parent: &mut AddrSpace) -> Result<(), Error> {
        log!(
            crate::LOG_DEF,
            "[{}] pager::clone(parent={})",
            self.id(),
            parent.id()
        );

        for ds in &mut parent.ds {
            let mut ds_idx = if let Some(cur) = self.find_ds_idx(ds.virt()) {
                // if the same dataspace does already exist, keep it and inherit it again
                if self.ds[cur].id() == ds.id() {
                    Some(cur)
                }
                // otherwise, remove it
                else {
                    self.ds.remove(cur);
                    None
                }
            }
            else {
                None
            };

            if ds_idx.is_none() {
                self.ds.push(ds.clone_for(self.owner.unwrap()));
                ds_idx.replace(self.ds.len() - 1);
            }

            self.ds[ds_idx.unwrap()].inherit(ds)?;
        }

        is.reply_error(Code::Success)
    }

    pub fn pagefault(
        &mut self,
        childs: &mut childs::ChildManager,
        is: &mut GateIStream<'_>,
    ) -> Result<(), Error> {
        let virt: goff = is.pop()?;
        let access = PageFlags::from_bits_truncate(is.pop()?) & !PageFlags::U;
        let access = Perm::from_bits_truncate(access.bits() as u32);

        log!(
            crate::LOG_DEF,
            "[{}] pager::pagefault(virt={:#x}, access={:#x})",
            self.id(),
            virt,
            access
        );

        if !self.has_owner() {
            log!(crate::LOG_DEF, "Invalid session");
            return Err(Error::new(Code::InvArgs));
        }

        self.pagefault_at(childs, virt, access)?;

        is.reply_error(Code::Success)
    }

    pub(crate) fn pagefault_at(
        &mut self,
        childs: &mut childs::ChildManager,
        virt: goff,
        access: Perm,
    ) -> Result<(), Error> {
        if let Some(ds) = self.find_ds_mut(virt) {
            if (ds.perm() & access) != access {
                log!(
                    crate::LOG_DEF,
                    "Access at {:#x} for {:#x} not allowed: {:#x}",
                    virt,
                    access,
                    ds.perm()
                );
                return Err(Error::new(Code::InvArgs));
            }

            ds.handle_pf(childs, virt)
        }
        else {
            log!(crate::LOG_DEF, "No dataspace at {:#x}", virt);
            Err(Error::new(Code::NotFound))
        }
    }

    pub fn map_ds(&mut self, args: &mut M3Deserializer<'_>) -> Result<(Selector, goff), Error> {
        if !self.has_owner() {
            return Err(Error::new(Code::InvArgs));
        }

        let virt = args.pop()?;
        let len = args.pop()?;
        let perm = Perm::from_bits_truncate(args.pop()?);
        let flags = MapFlags::from_bits_truncate(args.pop()?);
        let off = args.pop()?;

        let sel = Activity::own().alloc_sel();
        self.map_ds_with(virt, len, off, perm, flags, sel)
            .map(|virt| (sel, virt))
    }

    pub(crate) fn map_ds_with(
        &mut self,
        virt: goff,
        len: goff,
        off: goff,
        perm: Perm,
        flags: MapFlags,
        sess: Selector,
    ) -> Result<goff, Error> {
        log!(
            crate::LOG_DEF,
            "[{}] pager::map_ds(virt={:#x}, len={:#x}, perm={:?}, off={:#x}, flags={:?})",
            self.id(),
            virt,
            len,
            perm,
            off,
            flags,
        );

        self.check_map_args(virt, len, perm)?;

        let ds = DataSpace::new_extern(
            self.owner.unwrap(),
            self.child.unwrap(),
            virt,
            len,
            perm,
            flags,
            off,
            sess,
        );
        self.ds.push(ds);

        Ok(virt)
    }

    pub fn map_anon(&mut self, is: &mut GateIStream<'_>) -> Result<(), Error> {
        if !self.has_owner() {
            return Err(Error::new(Code::InvArgs));
        }

        let virt: goff = is.pop()?;
        let len: goff = is.pop()?;
        let perm = Perm::from_bits_truncate(is.pop::<u32>()?);
        let flags = MapFlags::from_bits_truncate(is.pop::<u32>()?);

        self.map_anon_with(virt, len, perm, flags)?;

        reply_vmsg!(is, Code::Success, virt)
    }

    pub(crate) fn map_anon_with(
        &mut self,
        virt: goff,
        len: goff,
        perm: Perm,
        flags: MapFlags,
    ) -> Result<(), Error> {
        log!(
            crate::LOG_DEF,
            "[{}] pager::map_anon(virt={:#x}, len={:#x}, perm={:?}, flags={:?})",
            self.id(),
            virt,
            len,
            perm,
            flags
        );

        self.check_map_args(virt, len, perm)?;

        let ds = DataSpace::new_anon(
            self.owner.unwrap(),
            self.child.unwrap(),
            virt,
            len,
            perm,
            flags,
        );
        self.ds.push(ds);

        Ok(())
    }

    pub fn map_mem(&mut self, args: &mut M3Deserializer<'_>) -> Result<(Selector, goff), Error> {
        if !self.has_owner() {
            return Err(Error::new(Code::InvArgs));
        }

        let virt: goff = args.pop()?;
        let len: goff = args.pop()?;
        let perm = Perm::from_bits_truncate(args.pop()?);

        log!(
            crate::LOG_DEF,
            "[{}] pager::map_mem(virt={:#x}, len={:#x}, perm={:?})",
            self.id(),
            virt,
            len,
            perm,
        );

        self.check_map_args(virt, len, perm)?;

        let mut ds = DataSpace::new_anon(
            self.owner.unwrap(),
            self.child.unwrap(),
            virt,
            len,
            perm,
            MapFlags::empty(),
        );

        // immediately insert a region, so that we don't allocate new memory on PFs
        let sel = Activity::own().alloc_sel();
        ds.populate(sel);

        self.ds.push(ds);

        Ok((sel, virt))
    }

    pub fn unmap(&mut self, is: &mut GateIStream<'_>) -> Result<(), Error> {
        let virt: goff = is.pop()?;

        log!(
            crate::LOG_DEF,
            "[{}] pager::unmap(virt={:#x})",
            self.id(),
            virt,
        );

        if let Some(idx) = self.find_ds_idx(virt) {
            self.ds.remove(idx);
        }
        else {
            log!(crate::LOG_DEF, "No dataspace at {:#x}", virt);
            return Err(Error::new(Code::NotFound));
        }

        is.reply_error(Code::Success)
    }

    pub fn close(&mut self, is: &mut GateIStream<'_>) -> Result<(), Error> {
        log!(crate::LOG_DEF, "[{}] pager::close()", self.id());

        is.reply_error(Code::Success)
    }

    fn check_map_args(&self, virt: goff, len: goff, perm: Perm) -> Result<(), Error> {
        if virt >= MAX_VIRT_ADDR {
            return Err(Error::new(Code::InvArgs));
        }
        if (virt & cfg::PAGE_BITS as goff) != 0 || (len & cfg::PAGE_BITS as goff) != 0 {
            return Err(Error::new(Code::InvArgs));
        }
        if perm.is_empty() {
            return Err(Error::new(Code::InvArgs));
        }
        if self.overlaps(virt, len) {
            return Err(Error::new(Code::Exists));
        }

        Ok(())
    }

    fn find_ds_mut(&mut self, virt: goff) -> Option<&mut DataSpace> {
        self.find_ds_idx(virt).map(move |idx| &mut self.ds[idx])
    }

    fn find_ds_idx(&self, virt: goff) -> Option<usize> {
        for (i, ds) in self.ds.iter().enumerate() {
            if virt >= ds.virt() && virt < ds.virt() + ds.size() {
                return Some(i);
            }
        }
        None
    }

    fn overlaps(&self, virt: goff, size: goff) -> bool {
        for ds in &self.ds {
            if math::overlaps(ds.virt(), ds.virt() + ds.size(), virt, virt + size) {
                return true;
            }
        }
        false
    }
}

impl Drop for AddrSpace {
    fn drop(&mut self) {
        // mark all regions in all dataspaces as not-mapped so that we don't needlessly revoke them.
        // the activity is destroyed anyway, therefore we don't need to do that.
        for ds in &mut self.ds {
            ds.kill();
        }
    }
}
