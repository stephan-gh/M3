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
use m3::cell::RefCell;
use m3::cfg;
use m3::col::Vec;
use m3::com::{GateIStream, MGateFlags, MemGate, SGateArgs, SendGate};
use m3::dtu::Label;
use m3::errors::{Code, Error};
use m3::goff;
use m3::kif::{PageFlags, syscalls::ExchangeArgs, Perm};
use m3::math;
use m3::pes::VPE;
use m3::rc::Rc;
use m3::server::SessId;
use m3::session::{MapFlags, ServerSession};

use dataspace::DataSpace;
use physmem::PhysMem;
use rgate;

const MAX_VIRT_ADDR: goff = cfg::MEM_CAP_END as goff - 1;

pub struct ASMem {
    pub vpe: Selector,
    pub mgate: MemGate,
}

pub struct AddrSpace {
    id: SessId,
    parent: Option<SessId>,
    _sess: ServerSession,
    as_mem: Option<Rc<ASMem>>,
    sgates: Vec<SendGate>,
    ds: Vec<Box<DataSpace>>,
}

impl AddrSpace {
    pub fn new(
        id: SessId,
        parent: Option<SessId>,
        srv_sel: Selector,
        sel: Selector,
    ) -> Result<Self, Error> {
        Ok(AddrSpace {
            id,
            parent,
            _sess: ServerSession::new_with_sel(srv_sel, sel, id as u64)?,
            as_mem: None,
            sgates: Vec::new(),
            ds: Vec::new(),
        })
    }

    pub fn parent(&self) -> Option<SessId> {
        self.parent
    }

    pub fn has_as_mem(&self) -> bool {
        self.as_mem.is_some()
    }

    pub fn init(&mut self, sels: Selector) {
        log!(crate::LOG_DEF, "[{}] pager::init(sels={})", self.id, sels);

        let mut mem = MemGate::new_bind(sels + 1);
        // we don't want to cause pagefault with this, because we are the one that handles them. we
        // will make sure that this doesn't happen by only accessing memory where we are sure that
        // we have mapped it.
        mem.set_flags(MGateFlags::NOPF);

        self.as_mem = Some(Rc::new(ASMem {
            vpe: sels + 0,
            mgate: mem,
        }));
    }

    pub fn add_sgate(&mut self) -> Result<Selector, Error> {
        log!(crate::LOG_DEF, "[{}] pager::add_sgate()", self.id);

        let sgate = SendGate::new_with(SGateArgs::new(rgate()).label(self.id as Label).credits(1))?;
        let sel = sgate.sel();
        self.sgates.push(sgate);

        Ok(sel)
    }

    pub fn clone(&mut self, is: &mut GateIStream, parent: &mut AddrSpace) -> Result<(), Error> {
        log!(crate::LOG_DEF, "[{}] pager::clone(parent={})", self.id, parent.id);

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
                let as_mem = self.as_mem.as_ref().unwrap().clone();
                let nds = Box::new(ds.clone_for(as_mem));
                self.ds.push(nds);
                ds_idx.replace(self.ds.len() - 1);
            }

            self.ds[ds_idx.unwrap()].inherit(ds)?;
        }

        reply_vmsg!(is, 0)
    }

    pub(crate) fn physmem_at(&self, virt: goff) -> Option<(goff, Rc<RefCell<PhysMem>>)> {
        self.find_ds(virt).and_then(|ds| ds.physmem_at(virt))
    }

    pub fn pagefault(&mut self, is: &mut GateIStream) -> Result<(), Error> {
        let virt: goff = is.pop();
        let access = PageFlags::from_bits_truncate(is.pop()) & !PageFlags::U;
        let access = Perm::from_bits_truncate(access.bits() as u32);

        log!(
            crate::LOG_DEF,
            "[{}] pager::pagefault(virt={:#x}, access={:#x})",
            self.id,
            virt,
            access
        );

        if !self.has_as_mem() {
            log!(crate::LOG_DEF, "Invalid session");
            return Err(Error::new(Code::InvArgs));
        }

        self.pagefault_at(virt, access)?;

        reply_vmsg!(is, 0)
    }

    pub(crate) fn pagefault_at(&mut self, virt: goff, access: Perm) -> Result<(), Error> {
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

            ds.handle_pf(virt)
        }
        else {
            log!(crate::LOG_DEF, "No dataspace at {:#x}", virt);
            Err(Error::new(Code::NotFound))
        }
    }

    pub fn map_ds(&mut self, args: &ExchangeArgs) -> Result<(Selector, goff), Error> {
        if args.count() != 6 || !self.has_as_mem() {
            return Err(Error::new(Code::InvArgs));
        }

        let virt = args.ival(1) as goff;
        let len = args.ival(2) as goff;
        let perm = Perm::from_bits_truncate(args.ival(3) as u32);
        let flags = MapFlags::from_bits_truncate(args.ival(4) as u32);
        let off = args.ival(5) as goff;

        let sel = VPE::cur().alloc_sel();
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
            self.id,
            virt,
            len,
            perm,
            off,
            flags,
        );

        self.check_map_args(virt, len, perm)?;

        let as_mem = self.as_mem.as_ref().unwrap().clone();
        let ds = Box::new(DataSpace::new_extern(
            as_mem, virt, len, perm, flags, off, sess,
        ));
        self.ds.push(ds);

        Ok(virt)
    }

    pub fn map_anon(&mut self, is: &mut GateIStream) -> Result<(), Error> {
        if !self.has_as_mem() {
            return Err(Error::new(Code::InvArgs));
        }

        let virt: goff = is.pop();
        let len: goff = is.pop();
        let perm = Perm::from_bits_truncate(is.pop::<u32>());
        let flags = MapFlags::from_bits_truncate(is.pop::<u32>());

        self.map_anon_with(virt, len, perm, flags)?;

        reply_vmsg!(is, 0, virt)
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
            self.id,
            virt,
            len,
            perm,
            flags
        );

        self.check_map_args(virt, len, perm)?;

        let as_mem = self.as_mem.as_ref().unwrap().clone();
        self.ds.push(Box::new(DataSpace::new_anon(
            as_mem, virt, len, perm, flags,
        )));

        Ok(())
    }

    pub fn map_mem(&mut self, args: &ExchangeArgs) -> Result<(Selector, goff), Error> {
        if args.count() != 4 || !self.has_as_mem() {
            return Err(Error::new(Code::InvArgs));
        }

        let virt = args.ival(1) as goff;
        let len = args.ival(2) as goff;
        let perm = Perm::from_bits_truncate(args.ival(3) as u32);

        log!(
            crate::LOG_DEF,
            "[{}] pager::map_mem(virt={:#x}, len={:#x}, perm={:?})",
            self.id,
            virt,
            len,
            perm,
        );

        self.check_map_args(virt, len, perm)?;

        let as_mem = self.as_mem.as_ref().unwrap().clone();
        let mut ds = Box::new(DataSpace::new_anon(
            as_mem,
            virt,
            len,
            perm,
            MapFlags::empty(),
        ));

        // immediately insert a region, so that we don't allocate new memory on PFs
        let sel = VPE::cur().alloc_sel();
        ds.populate(sel);

        self.ds.push(ds);

        Ok((sel, virt))
    }

    pub fn unmap(&mut self, is: &mut GateIStream) -> Result<(), Error> {
        let virt: goff = is.pop();

        log!(crate::LOG_DEF, "[{}] pager::unmap(virt={:#x})", self.id, virt,);

        if let Some(idx) = self.find_ds_idx(virt) {
            self.ds.remove(idx);
        }
        else {
            log!(crate::LOG_DEF, "No dataspace at {:#x}", virt);
            return Err(Error::new(Code::NotFound));
        }

        reply_vmsg!(is, 0)
    }

    pub fn close(&mut self, is: &mut GateIStream) -> Result<(), Error> {
        log!(crate::LOG_DEF, "[{}] pager::close()", self.id);

        reply_vmsg!(is, 0)
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

    fn find_ds(&self, virt: goff) -> Option<&Box<DataSpace>> {
        self.find_ds_idx(virt).map(move |idx| &self.ds[idx])
    }

    fn find_ds_mut(&mut self, virt: goff) -> Option<&mut Box<DataSpace>> {
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
        // the VPE is destroyed anyway, therefore we don't need to do that.
        for ds in &mut self.ds {
            ds.kill();
        }
    }
}
