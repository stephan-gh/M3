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

use core::cmp;
use core::fmt;
use m3::boxed::Box;
use m3::cap::Selector;
use m3::cell::RefCell;
use m3::cfg;
use m3::col::Vec;
use m3::com::MemGate;
use m3::errors::Error;
use m3::goff;
use m3::kif::{CapRngDesc, CapType, Perm, INVALID_SEL};
use m3::rc::Rc;
use m3::syscalls;

use addrspace::ASMem;
use physmem::{copy_block, PhysMem};

bitflags! {
    struct RegionFlags : u64 {
        const MAPPED = 0x1;
        const COW    = 0x2;
    }
}

pub struct Region {
    as_mem: Rc<ASMem>,
    mem: Option<Rc<RefCell<PhysMem>>>,
    mem_off: goff,
    ds_off: goff,
    off: goff,
    size: goff,
    perm: Perm,
    flags: RegionFlags,
}

impl Region {
    pub fn new(as_mem: Rc<ASMem>, ds_off: goff, off: goff, size: goff) -> Self {
        Region {
            as_mem,
            mem: None,
            mem_off: 0,
            ds_off,
            off,
            size,
            perm: Perm::empty(),
            flags: RegionFlags::empty(),
        }
    }

    pub fn clone_for(&self, as_mem: Rc<ASMem>) -> Self {
        Region {
            as_mem,
            mem: self.mem.clone(),
            mem_off: self.mem_off,
            ds_off: self.ds_off,
            off: self.off,
            size: self.size,
            perm: self.perm,
            flags: self.flags,
        }
    }

    pub fn virt(&self) -> goff {
        self.ds_off + self.off
    }

    pub fn offset(&self) -> goff {
        self.off
    }

    pub fn set_offset(&mut self, off: goff) {
        self.off = off;
    }

    pub fn mem_off(&self) -> goff {
        self.mem_off
    }

    pub fn set_mem_off(&mut self, off: goff) {
        self.mem_off = off;
    }

    pub fn size(&self) -> goff {
        self.size
    }

    pub fn set_size(&mut self, size: goff) {
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

    pub fn handle_cow(&mut self, ds_perms: Perm) -> Result<(), Error> {
        self.flags.remove(RegionFlags::COW);

        // writable memory needs to be copied
        if ds_perms.contains(Perm::W) {
            let nmem = {
                let mem = self.mem.as_ref().unwrap();

                // if we are the last one, we can just take the memory
                if Rc::strong_count(&mem) == 1 {
                    // we are the owner now
                    mem.borrow_mut().set_owner(self.as_mem.clone(), self.ds_off);
                    return Ok(());
                }

                let mut mem = mem.borrow_mut();

                // either copy from owner memory or the physical memory
                let (ogate, off, osel) = if let Some(omem) = mem.owner_mem() {
                    (&omem.mgate, mem.owner_virt() + self.off, omem.mgate.sel())
                }
                else {
                    (mem.gate(), self.mem_off, INVALID_SEL)
                };

                // allocate new memory for our copy
                let ngate = MemGate::new(self.size as usize, Perm::RWX)?;

                log!(
                    PAGER,
                    "Copying memory {:#x}..{:#x} from {} (we are {})",
                    self.ds_off + self.off,
                    self.ds_off + self.off + self.size - 1,
                    if mem.owner_mem().is_some() {
                        "owner"
                    }
                    else {
                        "origin"
                    },
                    if self.as_mem.mgate.sel() == osel {
                        "owner"
                    }
                    else {
                        "not owner"
                    },
                );

                copy_block(ogate, &ngate, off, self.size);

                // are we the owner?
                if self.as_mem.mgate.sel() == osel {
                    // give the others the new memory gate
                    let old = mem.replace_gate(ngate);
                    // there is no owner anymore
                    mem.remove_owner();
                    // give us the old memory with a new PhysMem object
                    Rc::new(RefCell::new(PhysMem::new_with_mem(
                        self.as_mem.clone(),
                        mem.owner_virt(),
                        old,
                    )))
                }
                else {
                    // the others keep the old mem; we take the new one
                    Rc::new(RefCell::new(PhysMem::new_with_mem(
                        self.as_mem.clone(),
                        self.ds_off,
                        ngate,
                    )))
                }
            };

            self.mem = Some(nmem);
        }

        Ok(())
    }

    pub fn limit_to(&mut self, pos: goff, pages: goff) {
        if self.size > pages * cfg::PAGE_SIZE as goff {
            let end = self.off + self.size;
            if pos > (pages / 2) * cfg::PAGE_SIZE as goff {
                self.off = cmp::max(self.off, pos - (pages / 2) * cfg::PAGE_SIZE as goff);
            }
            self.size = cmp::min(pages * cfg::PAGE_SIZE as goff, end - self.off);
        }
    }

    pub fn clear(&self) {
        self.mem.as_ref().unwrap().borrow().clear(self.size);
    }

    pub fn map(&mut self, perm: Perm) -> Result<(), Error> {
        if let Some(ref mem) = self.mem {
            syscalls::create_map(
                (self.virt() >> cfg::PAGE_BITS as goff) as Selector,
                self.as_mem.vpe,
                mem.borrow().gate().sel(),
                (self.mem_off >> cfg::PAGE_BITS as goff) as Selector,
                (self.size >> cfg::PAGE_BITS as goff) as Selector,
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
                self.as_mem.vpe,
                CapRngDesc::new(
                    CapType::MAPPING,
                    (self.virt() >> cfg::PAGE_BITS as goff) as Selector,
                    (self.size() >> cfg::PAGE_BITS as goff) as Selector,
                ),
                true,
            )
            .ok();
        }
    }
}

impl fmt::Debug for Region {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(
            fmt,
            "Region[{:#x}..{:#x} with {:#x}]",
            self.virt(),
            self.virt() + self.size() - 1,
            self.perm
        )
    }
}

pub struct RegionList {
    as_mem: Rc<ASMem>,
    ds_off: goff,
    size: goff,
    // put regions in Boxes to cheaply move them around
    regs: Vec<Box<Region>>,
}

impl RegionList {
    pub fn new(as_mem: Rc<ASMem>, ds_off: goff, size: goff) -> Self {
        RegionList {
            as_mem,
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

            let mut nreg = Box::new(r.clone_for(self.as_mem.clone()));

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
        let mut r = Box::new(Region::new(self.as_mem.clone(), self.ds_off, 0, self.size));
        r.set_mem(Rc::new(RefCell::new(PhysMem::new_bind(
            self.as_mem.clone(),
            self.ds_off,
            sel,
        ))));
        self.regs.push(r);
    }

    pub fn pagefault(&mut self, off: goff) -> &mut Region {
        let idx = self.do_pagefault(off);
        &mut self.regs[idx]
    }

    pub fn physmem_at(&self, off: goff) -> Option<(goff, Rc<RefCell<PhysMem>>)> {
        self.regs
            .iter()
            .find(|r| off >= r.off && off < r.off + r.size)
            .map(|r| (off - r.off, r.mem.as_ref().unwrap().clone()))
    }

    pub fn kill(&mut self) {
        for r in &mut self.regs {
            r.kill();
        }
    }

    fn do_pagefault(&mut self, off: goff) -> usize {
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
            self.as_mem.clone(),
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
