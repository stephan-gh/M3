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
use m3::client::MapFlags;
use m3::col::Vec;
use m3::com::GateIStream;
use m3::errors::{Code, Error};
use m3::io::LogFlags;
use m3::kif::{CapRngDesc, CapType, PageFlags, Perm};
use m3::log;
use m3::mem::{GlobOff, VirtAddr};
use m3::reply_vmsg;
use m3::server::{CapExchange, ClientManager, RequestSession, ServerSession, SessId};
use m3::tiles::Activity;
use m3::util::math;
use resmng::childs;

use crate::dataspace::DataSpace;

const MAX_VIRT_ADDR: VirtAddr = VirtAddr::new(cfg::MEM_CAP_END.as_raw() - 1);

pub struct AddrSpace {
    #[allow(unused)]
    parent: Option<SessId>,
    serv: ServerSession,
    child: Option<childs::Id>,
    owner: Option<Selector>,
    ds: Vec<DataSpace>,
}

impl RequestSession for AddrSpace {
    fn new(serv: ServerSession, _arg: &str) -> Result<Self, Error>
    where
        Self: Sized,
    {
        log!(LogFlags::PgReqs, "[{}] pager::open()", serv.id());
        Ok(AddrSpace::new(serv, None, None))
    }

    fn close(&mut self, _cli: &mut ClientManager<Self>, sid: SessId, _sub_ids: &mut Vec<SessId>)
    where
        Self: Sized,
    {
        log!(LogFlags::PgReqs, "[{}] closing session", sid);
    }
}

impl AddrSpace {
    pub fn new(serv: ServerSession, parent: Option<SessId>, child: Option<childs::Id>) -> Self {
        AddrSpace {
            parent,
            serv,
            child,
            owner: None,
            ds: Vec::new(),
        }
    }

    pub fn id(&self) -> SessId {
        self.serv.id()
    }

    pub fn child_id(&self) -> Option<childs::Id> {
        self.child
    }

    #[allow(unused)]
    pub fn parent(&self) -> Option<SessId> {
        self.parent
    }

    pub fn has_owner(&self) -> bool {
        self.owner.is_some() && self.child.is_some()
    }

    pub fn add_child(
        cli: &mut ClientManager<Self>,
        crt: usize,
        sid: SessId,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        let child_id = cli.get_mut(sid).unwrap().child_id();

        let (sel, _) = cli.add(crt, |_cli, serv| {
            log!(
                LogFlags::PgReqs,
                "[{}] pager::add_child(nsid={})",
                sid,
                serv.id()
            );
            Ok(AddrSpace::new(serv, Some(sid), child_id))
        })?;

        xchg.out_caps(CapRngDesc::new(CapType::Object, sel, 1));

        Ok(())
    }

    pub fn init(
        cli: &mut ClientManager<Self>,
        _crt: usize,
        sid: SessId,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        let aspace = cli.get_mut(sid).unwrap();
        let sel = aspace.do_init(None, None)?;

        xchg.out_caps(CapRngDesc::new(CapType::Object, sel, 1));
        Ok(())
    }

    pub fn do_init(
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
                LogFlags::PgReqs,
                "[{}] pager::init(child={:?}, act={})",
                self.id(),
                child,
                act
            );
            if let Some(c) = child {
                assert!(self.child.is_none());
                self.child = Some(c);
            }
            self.owner = Some(act);
            Ok(act)
        }
    }

    #[allow(unused)]
    pub fn clone(&mut self, is: &mut GateIStream<'_>, parent: &mut AddrSpace) -> Result<(), Error> {
        log!(
            LogFlags::PgReqs,
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
        let virt: VirtAddr = is.pop()?;
        let access = PageFlags::from_bits_truncate(is.pop()?) & !PageFlags::U;
        let access = Perm::from_bits_truncate(access.bits() as u32);

        log!(
            LogFlags::PgReqs,
            "[{}] pager::pagefault(virt={}, access={:#x})",
            self.id(),
            virt,
            access
        );

        if !self.has_owner() {
            log!(LogFlags::Error, "Invalid session");
            return Err(Error::new(Code::InvArgs));
        }

        self.pagefault_at(childs, virt, access)?;

        is.reply_error(Code::Success)
    }

    pub(crate) fn pagefault_at(
        &mut self,
        childs: &mut childs::ChildManager,
        virt: VirtAddr,
        access: Perm,
    ) -> Result<(), Error> {
        if let Some(ds) = self.find_ds_mut(virt) {
            if (ds.perm() & access) != access {
                log!(
                    LogFlags::Error,
                    "Access at {} for {:#x} not allowed: {:#x}",
                    virt,
                    access,
                    ds.perm()
                );
                return Err(Error::new(Code::InvArgs));
            }

            ds.handle_pf(childs, virt)
        }
        else {
            log!(LogFlags::Error, "No dataspace at {}", virt);
            Err(Error::new(Code::NotFound))
        }
    }

    pub fn map_ds(
        cli: &mut ClientManager<Self>,
        _crt: usize,
        sid: SessId,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        let aspace = cli.get_mut(sid).unwrap();
        if !aspace.has_owner() {
            return Err(Error::new(Code::InvArgs));
        }

        let args = xchg.in_args();
        let virt = args.pop()?;
        let len = args.pop()?;
        let perm = Perm::from_bits_truncate(args.pop()?);
        let flags = MapFlags::from_bits_truncate(args.pop()?);
        let off = args.pop()?;

        let sel = Activity::own().alloc_sel();
        let virt = aspace.map_ds_with(virt, len, off, perm, flags, sel)?;

        xchg.out_args().push(virt);
        xchg.out_caps(CapRngDesc::new(CapType::Object, sel, 1));

        Ok(())
    }

    pub(crate) fn map_ds_with(
        &mut self,
        virt: VirtAddr,
        len: GlobOff,
        off: GlobOff,
        perm: Perm,
        flags: MapFlags,
        sess: Selector,
    ) -> Result<VirtAddr, Error> {
        log!(
            LogFlags::PgReqs,
            "[{}] pager::map_ds(virt={}, len={:#x}, perm={:?}, off={:#x}, flags={:?})",
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

        let virt: VirtAddr = is.pop()?;
        let len: GlobOff = is.pop()?;
        let perm = Perm::from_bits_truncate(is.pop::<u32>()?);
        let flags = MapFlags::from_bits_truncate(is.pop::<u32>()?);

        self.map_anon_with(virt, len, perm, flags)?;

        reply_vmsg!(is, Code::Success, virt)
    }

    pub(crate) fn map_anon_with(
        &mut self,
        virt: VirtAddr,
        len: GlobOff,
        perm: Perm,
        flags: MapFlags,
    ) -> Result<(), Error> {
        log!(
            LogFlags::PgReqs,
            "[{}] pager::map_anon(virt={}, len={:#x}, perm={:?}, flags={:?})",
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

    pub fn map_mem(
        cli: &mut ClientManager<Self>,
        _crt: usize,
        sid: SessId,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        let aspace = cli.get_mut(sid).unwrap();
        if !aspace.has_owner() {
            return Err(Error::new(Code::InvArgs));
        }

        let args = xchg.in_args();
        let virt: VirtAddr = args.pop()?;
        let len: GlobOff = args.pop()?;
        let perm = Perm::from_bits_truncate(args.pop()?);

        log!(
            LogFlags::PgReqs,
            "[{}] pager::map_mem(virt={}, len={:#x}, perm={:?})",
            aspace.id(),
            virt,
            len,
            perm,
        );

        aspace.check_map_args(virt, len, perm)?;

        let mut ds = DataSpace::new_anon(
            aspace.owner.unwrap(),
            aspace.child.unwrap(),
            virt,
            len,
            perm,
            MapFlags::empty(),
        );

        // immediately insert a region, so that we don't allocate new memory on PFs
        let sel = Activity::own().alloc_sel();
        ds.populate(sel);

        aspace.ds.push(ds);

        xchg.out_args().push(virt);
        xchg.out_caps(CapRngDesc::new(CapType::Object, sel, 1));

        Ok(())
    }

    pub fn unmap(&mut self, is: &mut GateIStream<'_>) -> Result<(), Error> {
        let virt: VirtAddr = is.pop()?;

        log!(
            LogFlags::PgReqs,
            "[{}] pager::unmap(virt={})",
            self.id(),
            virt,
        );

        if let Some(idx) = self.find_ds_idx(virt) {
            self.ds.remove(idx);
        }
        else {
            log!(LogFlags::Error, "No dataspace at {}", virt);
            return Err(Error::new(Code::NotFound));
        }

        is.reply_error(Code::Success)
    }

    fn check_map_args(&self, virt: VirtAddr, len: GlobOff, perm: Perm) -> Result<(), Error> {
        if virt >= MAX_VIRT_ADDR {
            return Err(Error::new(Code::InvArgs));
        }
        if !(virt & VirtAddr::from(cfg::PAGE_BITS)).is_null()
            || (len & cfg::PAGE_BITS as GlobOff) != 0
        {
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

    fn find_ds_mut(&mut self, virt: VirtAddr) -> Option<&mut DataSpace> {
        self.find_ds_idx(virt).map(move |idx| &mut self.ds[idx])
    }

    fn find_ds_idx(&self, virt: VirtAddr) -> Option<usize> {
        for (i, ds) in self.ds.iter().enumerate() {
            if virt >= ds.virt() && virt < ds.virt() + ds.size() {
                return Some(i);
            }
        }
        None
    }

    fn overlaps(&self, virt: VirtAddr, size: GlobOff) -> bool {
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
