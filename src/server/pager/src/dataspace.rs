/*
 * Copyright (C) 2019-2021 Nils Asmussen, Barkhausen Institut
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
use m3::cell::{RefCell, StaticCell};
use m3::cfg;
use m3::client::{ClientSession, MapFlags, M3FS};
use m3::com::MemGate;
use m3::errors::{Code, Error};
use m3::io::LogFlags;
use m3::kif;
use m3::log;
use m3::mem::{GlobOff, VirtAddr};
use m3::rc::Rc;
use m3::util::math;
use resmng::childs;

use crate::physmem::PhysMem;
use crate::regions::RegionList;

const MAX_ANON_PAGES: usize = 4;
const MAX_EXT_PAGES: usize = 8;

static NEXT_ID: StaticCell<u64> = StaticCell::new(0);

fn alloc_id() -> u64 {
    let id = NEXT_ID.get();
    NEXT_ID.set(id + 1);
    id
}

struct FileMapping {
    sess: ClientSession,
    offset: GlobOff,
}

impl FileMapping {
    fn new(sel: Selector, offset: GlobOff) -> Self {
        FileMapping {
            sess: ClientSession::new_bind(sel),
            offset,
        }
    }
}

impl Clone for FileMapping {
    fn clone(&self) -> Self {
        FileMapping {
            sess: ClientSession::new_bind(self.sess.sel()),
            offset: self.offset,
        }
    }
}

pub struct DataSpace {
    id: u64,
    child: childs::Id,
    virt: VirtAddr,
    size: GlobOff,
    perms: kif::Perm,
    flags: MapFlags,
    regions: RegionList,
    owner: Selector,
    file: Option<FileMapping>,
}

impl DataSpace {
    #[allow(clippy::too_many_arguments)]
    pub fn new_extern(
        owner: Selector,
        child: childs::Id,
        virt: VirtAddr,
        size: GlobOff,
        perms: kif::Perm,
        flags: MapFlags,
        off: GlobOff,
        sel: Selector,
    ) -> Self {
        DataSpace {
            id: alloc_id(),
            child,
            virt,
            size,
            perms,
            flags,
            owner,
            regions: RegionList::new(owner, child, virt, size),
            file: Some(FileMapping::new(sel, off)),
        }
    }

    pub fn new_anon(
        owner: Selector,
        child: childs::Id,
        virt: VirtAddr,
        size: GlobOff,
        perms: kif::Perm,
        flags: MapFlags,
    ) -> Self {
        DataSpace {
            id: alloc_id(),
            child,
            virt,
            size,
            perms,
            flags,
            owner,
            regions: RegionList::new(owner, child, virt, size),
            file: None,
        }
    }

    pub fn clone_for(&self, owner: Selector) -> Self {
        DataSpace {
            id: self.id,
            child: self.child,
            virt: self.virt,
            size: self.size,
            perms: self.perms,
            flags: self.flags,
            owner,
            regions: RegionList::new(owner, self.child, self.virt, self.size),
            file: self.file.clone(),
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn virt(&self) -> VirtAddr {
        self.virt
    }

    pub fn size(&self) -> GlobOff {
        self.size
    }

    pub fn perm(&self) -> kif::Perm {
        self.perms
    }

    pub fn inherit(&mut self, ds: &mut DataSpace) -> Result<(), Error> {
        self.id = ds.id;

        // if it's not writable, but we have already regions, we can simply keep them
        if !ds.perms.contains(kif::Perm::W) && !self.regions.is_empty() {
            return Ok(());
        }

        let ds_perm = ds.perm();
        self.regions.clone(&mut ds.regions, ds_perm)
    }

    pub fn populate(&mut self, sel: Selector) {
        self.regions.populate(sel);
    }

    pub fn handle_pf(
        &mut self,
        childs: &mut childs::ChildManager,
        virt: VirtAddr,
    ) -> Result<(), Error> {
        let pf_off = math::round_dn((virt - self.virt).as_goff(), cfg::PAGE_SIZE as GlobOff);
        let reg = self.regions.pagefault(pf_off);

        // if it isn't backed with memory yet, allocate memory for it
        if !reg.has_mem() {
            if let Some(ref f) = self.file {
                // get memory cap for the region
                // TODO add a cache for that; we request the same caps over and over again
                let (off, len, sel) = M3FS::get_mem(&f.sess, f.offset + pf_off)?;

                // first, resize the region to not be too large
                reg.limit_to(pf_off, MAX_EXT_PAGES as GlobOff);

                // now, align the region with the memory capability that we got
                let cap_begin = f.offset + pf_off - off;
                // if it starts before the region, just remember this offset in the region
                if cap_begin < f.offset + reg.offset() {
                    reg.set_mem_off(f.offset + reg.offset() - cap_begin);
                }
                // otherwise, let the region start at the capability
                else {
                    let old_off = reg.offset();
                    reg.set_offset(cap_begin - f.offset);
                    reg.set_size(reg.size() - (reg.offset() - old_off));
                }

                // ensure that we don't exceed the memcap size
                if reg.mem_off() + reg.size() > len {
                    reg.set_size(math::round_up(
                        len - reg.mem_off(),
                        cfg::PAGE_SIZE as GlobOff,
                    ));
                }

                // if it's writable and should not be shared, create a copy
                if !self.flags.contains(MapFlags::SHARED) && self.perms.contains(kif::Perm::W) {
                    let src = MemGate::new_owned_bind(sel)?;
                    let child = childs
                        .child_by_id_mut(self.child)
                        .ok_or_else(|| Error::new(Code::ActivityGone))?;
                    // TODO this memory is currently only free'd on child exit
                    let (mgate, _alloc) = child.alloc_local(reg.size(), kif::Perm::RWX)?;
                    let mem = Rc::new(RefCell::new(PhysMem::new((self.owner, self.virt), mgate)?));
                    reg.set_mem(mem);
                    reg.copy_from(&src)?;
                    reg.set_mem_off(0);
                }
                else {
                    reg.set_mem(Rc::new(RefCell::new(PhysMem::new_bind(
                        (self.owner, self.virt),
                        sel,
                    ))));
                }

                log!(
                    LogFlags::PgMem,
                    "Obtained memory for {}..{}",
                    reg.virt(),
                    reg.virt() + reg.size() - 1
                );
            }
            else {
                let max = if !self.flags.contains(MapFlags::NOLPAGE)
                    && math::is_aligned(virt, VirtAddr::from(cfg::LPAGE_SIZE))
                    && reg.size() >= cfg::LPAGE_SIZE as GlobOff
                {
                    cfg::LPAGE_SIZE / cfg::PAGE_SIZE
                }
                else {
                    MAX_ANON_PAGES
                };

                // don't allocate too much at once
                reg.limit_to(pf_off, max as GlobOff);

                log!(
                    LogFlags::PgMem,
                    "Allocating anonymous memory for {}..{}",
                    reg.virt(),
                    reg.virt() + reg.size() - 1
                );

                let child = childs
                    .child_by_id_mut(self.child)
                    .ok_or_else(|| Error::new(Code::ActivityGone))?;
                // TODO this memory is currently only free'd on child exit
                let (mgate, _alloc) = child.alloc_local(reg.size(), kif::Perm::RWX)?;
                reg.set_mem(Rc::new(RefCell::new(PhysMem::new(
                    (self.owner, self.virt),
                    mgate,
                )?)));

                if !self.flags.contains(MapFlags::UNINIT) {
                    // zero the memory
                    reg.clear()?;
                }
            }
        }
        // if we have memory, but COW is in progress
        else if reg.is_cow() {
            reg.handle_cow(childs, self.perms)?;
        }
        else if reg.is_mapped() {
            // nothing to do
            return Ok(());
        }

        reg.map(self.perms)
    }

    pub fn kill(&mut self) {
        self.regions.kill();
    }
}
