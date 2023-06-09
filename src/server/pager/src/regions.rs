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

use bitflags::bitflags;
use core::cmp;
use core::fmt;
use m3::boxed::Box;
use m3::cap::Selector;
use m3::cell::RefCell;
use m3::cfg;
use m3::col::Vec;
use m3::com::MemGate;
use m3::errors::{Code, Error};
use m3::io::LogFlags;
use m3::kif::{CapRngDesc, CapType, Perm, INVALID_SEL};
use m3::log;
use m3::mem::{GlobOff, VirtAddr};
use m3::rc::Rc;
use m3::syscalls;
use resmng::childs;

use crate::physmem::{copy_block, PhysMem};

bitflags! {
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    struct RegionFlags : u64 {
        const MAPPED = 0x1;
        const COW    = 0x2;
    }
}

pub struct Region {
    owner: Selector,
    child: childs::Id,
    mem: Option<Rc<RefCell<PhysMem>>>,
    mem_off: GlobOff,
    ds_off: VirtAddr,
    off: GlobOff,
    size: GlobOff,
    perm: Perm,
    flags: RegionFlags,
}

impl Region {
    pub fn new(
        owner: Selector,
        child: childs::Id,
        ds_off: VirtAddr,
        off: GlobOff,
        size: GlobOff,
    ) -> Self {
        Region {
            owner,
            child,
            mem: None,
            mem_off: 0,
            ds_off,
            off,
            size,
            perm: Perm::empty(),
            flags: RegionFlags::empty(),
        }
    }

    pub fn clone_for(&self, owner: Selector) -> Self {
        Region {
            owner,
            child: self.child,
            mem: self.mem.clone(),
            mem_off: self.mem_off,
            ds_off: self.ds_off,
            off: self.off,
            size: self.size,
            perm: self.perm,
            flags: self.flags,
        }
    }

    pub fn virt(&self) -> VirtAddr {
        self.ds_off + VirtAddr::new(self.off)
    }

    pub fn offset(&self) -> GlobOff {
        self.off
    }

    pub fn set_offset(&mut self, off: GlobOff) {
        self.off = off;
    }

    pub fn mem_off(&self) -> GlobOff {
        self.mem_off
    }

    pub fn set_mem_off(&mut self, off: GlobOff) {
        self.mem_off = off;
    }

    pub fn size(&self) -> GlobOff {
        self.size
    }

    pub fn set_size(&mut self, size: GlobOff) {
        self.size = size;
    }

    pub fn has_mem(&self) -> bool {
        self.mem.is_some()
    }

    pub fn set_mem(&mut self, mem: Rc<RefCell<PhysMem>>) {
        self.mem = Some(mem);
    }

    pub fn is_mapped(&self) -> bool {
        self.flags.contains(RegionFlags::MAPPED)
    }

    pub fn is_cow(&self) -> bool {
        self.flags.contains(RegionFlags::COW)
    }

    pub fn handle_cow(
        &mut self,
        childs: &mut childs::ChildManager,
        ds_perms: Perm,
    ) -> Result<(), Error> {
        self.flags.remove(RegionFlags::COW);

        // writable memory needs to be copied
        if ds_perms.contains(Perm::W) {
            let nmem = {
                let mem = self.mem.as_ref().unwrap();

                // if we are the last one, we can just take the memory
                if Rc::strong_count(mem) == 1 {
                    // we are the owner now
                    mem.borrow_mut().set_owner(self.owner, self.ds_off);
                    return Ok(());
                }

                let mut mem = mem.borrow_mut();

                // either copy from owner memory or the physical memory
                let (off, osel) = if let Some((oact, ovirt)) = mem.owner_mem() {
                    ((ovirt + self.off).as_goff(), oact)
                }
                else {
                    (self.mem_off, INVALID_SEL)
                };

                // allocate new memory for our copy
                let child = childs
                    .child_by_id_mut(self.child)
                    .ok_or_else(|| Error::new(Code::ActivityGone))?;
                // TODO this memory is currently only free'd on child exit
                let (mut ngate, _alloc) = child.alloc_local(self.size, Perm::RWX)?;

                log!(
                    LogFlags::PgMem,
                    "Copying memory {}..{} from {} (we are {})",
                    self.virt(),
                    self.virt() + self.size - 1,
                    if mem.owner_mem().is_some() {
                        "owner"
                    }
                    else {
                        "origin"
                    },
                    if self.owner == osel {
                        "owner"
                    }
                    else {
                        "not owner"
                    },
                );

                if osel == INVALID_SEL {
                    copy_block(mem.gate(), &ngate, off, self.size);
                }
                else {
                    let omem = MemGate::new_foreign(osel, VirtAddr::new(off), self.size, Perm::R)?;
                    copy_block(&omem, &ngate, 0, self.size);
                }

                // are we the owner?
                if self.owner == osel {
                    // deactivate the MemGate, because we'll probably not need it again
                    ngate.deactivate();

                    // give the others the new memory gate
                    let old = mem.replace_gate(ngate);
                    let owner_virt = mem.owner_mem().unwrap().1;
                    // there is no owner anymore
                    mem.remove_owner();
                    // give us the old memory with a new PhysMem object
                    Rc::new(RefCell::new(PhysMem::new_with_mem(
                        (self.owner, owner_virt),
                        old,
                    )))
                }
                else {
                    // the others keep the old mem; we take the new one
                    Rc::new(RefCell::new(PhysMem::new_with_mem(
                        (self.owner, self.ds_off),
                        ngate,
                    )))
                }
            };

            // it's not that likely that we'll use this gate again, so deactivate it
            nmem.borrow_mut().deactivate();
            self.mem = Some(nmem);
        }

        Ok(())
    }

    pub fn limit_to(&mut self, pos: GlobOff, pages: GlobOff) {
        if self.size > pages * cfg::PAGE_SIZE as GlobOff {
            let end = self.off + self.size;
            if pos > (pages / 2) * cfg::PAGE_SIZE as GlobOff {
                self.off = cmp::max(self.off, pos - (pages / 2) * cfg::PAGE_SIZE as GlobOff);
            }
            self.size = cmp::min(pages * cfg::PAGE_SIZE as GlobOff, end - self.off);
        }
    }

    pub fn copy_from(&self, src: &MemGate) {
        if let Some(ref mem) = self.mem {
            copy_block(src, mem.borrow().gate(), self.mem_off, self.size());
            // see above
            mem.borrow_mut().deactivate();
        }
    }

    pub fn clear(&self) {
        let mem = self.mem.as_ref().unwrap();
        mem.borrow().clear(self.size);
        // see above
        mem.borrow_mut().deactivate();
    }

    pub fn map(&mut self, perm: Perm) -> Result<(), Error> {
        if let Some(ref mem) = self.mem {
            syscalls::create_map(
                self.virt(),
                self.owner,
                mem.borrow().gate().sel(),
                (self.mem_off >> cfg::PAGE_BITS as GlobOff) as Selector,
                (self.size as usize >> cfg::PAGE_BITS) as Selector,
                perm,
            )?;
            self.flags.insert(RegionFlags::MAPPED);
        }

        Ok(())
    }

    pub fn kill(&mut self) {
        // don't revoke the mapping caps, if the address space got destroyed
        self.flags.remove(RegionFlags::MAPPED);
    }
}

impl Drop for Region {
    fn drop(&mut self) {
        if self.mem.is_some() && self.flags.contains(RegionFlags::MAPPED) {
            syscalls::revoke(
                self.owner,
                CapRngDesc::new(
                    CapType::Mapping,
                    (self.virt().as_goff() >> cfg::PAGE_BITS as GlobOff) as Selector,
                    (self.size() >> cfg::PAGE_BITS as GlobOff) as Selector,
                ),
                true,
            )
            .ok();
        }
    }
}

impl fmt::Debug for Region {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            fmt,
            "Region[{}..{} with {:#x}]",
            self.virt(),
            self.virt() + self.size() - 1,
            self.perm
        )
    }
}

pub struct RegionList {
    owner: Selector,
    child: childs::Id,
    ds_off: VirtAddr,
    size: GlobOff,
    // put regions in Boxes to cheaply move them around
    #[allow(clippy::vec_box)]
    regs: Vec<Box<Region>>,
}

impl RegionList {
    pub fn new(owner: Selector, child: childs::Id, ds_off: VirtAddr, size: GlobOff) -> Self {
        RegionList {
            owner,
            child,
            ds_off,
            size,
            regs: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.regs.is_empty()
    }

    pub fn clone(&mut self, rl: &mut RegionList, ds_perms: Perm) -> Result<(), Error> {
        // for the case that we already have regions and the DS is writable, just remove them.
        // because there is no point in trying to keep them:
        // 1. we have already our own copy
        //    -> then we need to revoke that and create a new one anyway
        // 2. COW is still set
        //    -> then we would save the object copying, but this is not that expensive
        // in general, if we try to keep them, we need to match the region lists against each other,
        // which is probably more expensive than just destructing and creating a few objects
        self.regs.clear();

        for r in &mut rl.regs {
            // make it readonly, if it's writable and we have not done that yet
            if !r.is_cow() && ds_perms.contains(Perm::W) {
                r.map(ds_perms ^ Perm::W)?;
            }

            let mut nreg = Box::new(r.clone_for(self.owner));

            // adjust flags
            if ds_perms.contains(Perm::W) {
                r.flags.insert(RegionFlags::COW);
            }
            // for the clone, even readonly regions are mapped on demand
            nreg.flags.insert(RegionFlags::COW);
            self.regs.push(nreg);
        }
        Ok(())
    }

    pub fn populate(&mut self, sel: Selector) {
        assert!(self.regs.is_empty());
        let mut r = Box::new(Region::new(
            self.owner,
            self.child,
            self.ds_off,
            0,
            self.size,
        ));
        r.set_mem(Rc::new(RefCell::new(PhysMem::new_bind(
            (self.owner, self.ds_off),
            sel,
        ))));
        self.regs.push(r);
    }

    pub fn pagefault(&mut self, off: GlobOff) -> &mut Region {
        let idx = self.do_pagefault(off);
        &mut self.regs[idx]
    }

    pub fn kill(&mut self) {
        for r in &mut self.regs {
            r.kill();
        }
    }

    fn do_pagefault(&mut self, off: GlobOff) -> usize {
        // search for the region that contains `off` or is behind `off`
        let mut last = None;
        let mut idx = 0;
        while idx < self.regs.len() {
            if self.regs[idx].off + self.regs[idx].size > off {
                break;
            }
            last = Some(idx);
            idx += 1;
        }

        if idx != self.regs.len() {
            let nreg = &mut self.regs[idx];
            // does it contain `off`?
            if off >= nreg.off && off < nreg.off + nreg.size {
                return idx;
            }
        }

        // build a new region that spans from the previous one to the next one
        let start = if let Some(l) = last {
            self.regs[l].off + self.regs[l].size
        }
        else {
            0
        };
        let end = if idx == self.regs.len() {
            self.size
        }
        else {
            self.regs[idx].off
        };

        // insert region
        let r = Box::new(Region::new(
            self.owner,
            self.child,
            self.ds_off,
            start,
            end - start,
        ));
        let nidx = match last {
            Some(n) => n + 1,
            None => 0,
        };
        self.regs.insert(nidx, r);
        nidx
    }
}
