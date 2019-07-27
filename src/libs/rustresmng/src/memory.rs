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

use core::fmt;
use m3::cfg;
use m3::cap::Selector;
use m3::cell::StaticCell;
use m3::col::Vec;
use m3::com::MemGate;
use m3::errors::{Code, Error};
use m3::goff;
use m3::kif::Perm;
use m3::mem::MemMap;

use childs::Child;

pub struct MemMod {
    gate: MemGate,
    size: usize,
    map: MemMap,
    reserved: bool,
}

impl MemMod {
    pub fn new(sel: Selector, size: usize, reserved: bool) -> Self {
        MemMod {
            gate: MemGate::new_bind(sel),
            size,
            map: MemMap::new(0, size),
            reserved,
        }
    }

    pub fn capacity(&self) -> usize {
        self.size
    }
    pub fn available(&self) -> usize {
        self.map.size().0
    }
}

impl fmt::Debug for MemMod {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MemMod[sel: {}, res: {}, size: {} MiB, available: {} MiB, map: {:?}]",
            self.gate.sel(), self.reserved,
            self.size / (1024 * 1024), self.map.size().0 / (1024 * 1024),
            self.map)
    }
}

pub struct MainMemory {
    mods: Vec<MemMod>,
}

pub struct Allocation {
    pub mod_id: usize,
    pub addr: goff,
    pub size: usize,
    pub sel: Selector,
}

impl Allocation {
    pub fn new(mod_id: usize, addr: goff, size: usize, sel: Selector) -> Self {
        Allocation { mod_id, addr, size, sel }
    }
}

impl Drop for Allocation {
    fn drop(&mut self) {
        log!(RESMNG_MEM, "Freeing {:?}", self);
        get().mods[self.mod_id].map.free(self.addr, self.size);
    }
}

impl fmt::Debug for Allocation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Alloc[mod={}, addr={:#x}, size={:#x}, sel={}]",
               self.mod_id, self.addr, self.size, self.sel)
    }
}

impl MainMemory {
    const fn new() -> Self {
        MainMemory {
            mods: Vec::new(),
        }
    }

    pub fn capacity(&self) -> usize {
        self.mods.iter().fold(0, |total, ref m| total + m.capacity())
    }
    pub fn available(&self) -> usize {
        self.mods.iter().fold(0, |total, ref m| total + m.available())
    }

    pub fn mem_cap(&self, idx: usize) -> Selector {
        self.mods[idx].gate.sel()
    }

    pub fn add(&mut self, m: MemMod) {
        self.mods.push(m)
    }

    pub fn allocate(&mut self, size: usize) -> Result<Allocation, Error> {
        let align = if size >= cfg::LPAGE_SIZE { cfg::LPAGE_SIZE } else { cfg::PAGE_SIZE };

        for (id, m) in &mut self.mods.iter_mut().enumerate() {
            if m.reserved {
                continue;
            }

            if let Ok(addr) = m.map.allocate(size, align) {
                let alloc = Allocation::new(id, addr, size, 0);
                log!(RESMNG_MEM, "Allocated {:?}", alloc);
                return Ok(alloc);
            }
        }
        Err(Error::new(Code::OutOfMem))
    }

    pub fn allocate_for(&mut self, child: &mut dyn Child, dst_sel: Selector,
                        size: usize, perm: Perm) -> Result<(), Error> {
        log!(RESMNG_MEM, "{}: allocate(dst_sel={}, size={:#x}, perm={:?})",
             child.name(), dst_sel, size, perm);

        let mut alloc = self.allocate(size)?;
        let mod_id = alloc.mod_id;
        alloc.sel = dst_sel;
        child.add_mem(alloc, self.mods[mod_id].gate.sel(), perm)
    }

    pub fn allocate_at(&mut self, child: &mut dyn Child, dst_sel: Selector,
                       offset: goff, size: usize) -> Result<(), Error> {
        log!(RESMNG_MEM, "{}: allocate_at(dst_sel={}, offset={:#x}, size={:#x})",
             child.name(), dst_sel, offset, size);

        // TODO check if that's actually ok
        let m = &self.mods[0];
        assert!(m.reserved);
        child.add_mem(Allocation::new(0, offset, size, dst_sel), m.gate.sel(), Perm::RWX)
    }
}

impl fmt::Debug for MainMemory {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "size: {} MiB, available: {} MiB, mods: [",
            self.capacity() / (1024 * 1024), self.available() / (1024 * 1024))?;
        for m in &self.mods {
            writeln!(f, "  {:?}", m)?;
        }
        write!(f, "]")
    }
}

static MEM: StaticCell<MainMemory> = StaticCell::new(MainMemory::new());

pub fn get() -> &'static mut MainMemory {
    MEM.get_mut()
}
