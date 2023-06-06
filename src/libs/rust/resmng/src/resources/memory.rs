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

use core::cmp;
use core::fmt;
use m3::cap::Selector;
use m3::cfg;
use m3::col::Vec;
use m3::com::MemGate;
use m3::errors::{Code, Error};
use m3::goff;
use m3::io::LogFlags;
use m3::kif::Perm;
use m3::log;
use m3::mem::{GlobAddr, MemMap};
use m3::rc::Rc;
use m3::util::math;

pub struct MemMod {
    gate: MemGate,
    addr: GlobAddr,
    size: goff,
    reserved: bool,
}

impl MemMod {
    pub fn new(gate: MemGate, addr: GlobAddr, size: goff, reserved: bool) -> Self {
        MemMod {
            gate,
            addr,
            size,
            reserved,
        }
    }

    pub fn mgate(&self) -> &MemGate {
        &self.gate
    }

    pub fn addr(&self) -> GlobAddr {
        self.addr
    }

    pub fn capacity(&self) -> goff {
        self.size
    }
}

impl fmt::Debug for MemMod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MemMod[sel: {}, res: {}, addr: {:?}, size: {} MiB]",
            self.gate.sel(),
            self.reserved,
            self.addr,
            self.size / (1024 * 1024),
        )
    }
}

#[derive(Default)]
pub struct MemoryManager {
    mods: Vec<Rc<MemMod>>,
    available: goff,
    cur_mod: usize,
    cur_off: goff,
}

impl MemoryManager {
    pub fn mods(&self) -> &[Rc<MemMod>] {
        &self.mods
    }

    pub fn add(&mut self, m: Rc<MemMod>) {
        if !m.reserved {
            self.available += m.capacity();
        }
        self.mods.push(m);
    }

    pub fn capacity(&self) -> goff {
        self.mods.iter().fold(0, |total, m| {
            if !m.reserved {
                total + m.capacity()
            }
            else {
                total
            }
        })
    }

    pub fn available(&self) -> goff {
        self.available
    }

    pub fn find_mem(&self, addr: GlobAddr, size: goff, perm: Perm) -> Result<MemSlice, Error> {
        for m in &self.mods {
            if addr.tile() == m.addr.tile()
                && addr.offset() >= m.addr.offset()
                && addr.offset() + size <= m.addr.offset() + m.capacity()
            {
                return Ok(MemSlice::new(
                    m.clone(),
                    addr.offset() - m.addr.offset(),
                    size,
                    perm,
                ));
            }
        }
        Err(Error::new(Code::InvArgs))
    }

    pub fn alloc_mem(&mut self, mut size: goff) -> Result<MemSlice, Error> {
        size = math::round_up(size, cfg::PAGE_SIZE as goff);
        while self.cur_mod < self.mods.len() {
            if let Some(sl) = self.get_slice(size, true) {
                self.cur_off += sl.size;
                self.available -= sl.size;
                return Ok(sl);
            }
        }
        Err(Error::new(Code::NoSpace))
    }

    pub fn alloc_pool(&mut self, mut size: goff) -> Result<MemPool, Error> {
        let mut res = MemPool::default();
        size = math::round_up(size, cfg::PAGE_SIZE as goff);
        while size > 0 && self.cur_mod < self.mods.len() {
            if let Some(sl) = self.get_slice(size, false) {
                size -= sl.size;
                self.cur_off += sl.size;
                self.available -= sl.size;
                res.add(sl);
            }
        }

        if size == 0 {
            Ok(res)
        }
        else {
            Err(Error::new(Code::NoSpace))
        }
    }

    fn get_slice(&mut self, size: goff, all: bool) -> Option<MemSlice> {
        let m = &self.mods[self.cur_mod];
        let avail = m.capacity() - self.cur_off;
        if m.reserved || self.cur_off == m.capacity() || (all && avail < size) {
            self.cur_mod += 1;
            self.cur_off = 0;
            if !m.reserved {
                self.available -= avail;
            }
            return None;
        }

        let amount = cmp::min(avail, size);
        Some(MemSlice::new(m.clone(), self.cur_off, amount, Perm::RWX))
    }
}

pub struct MemSlice {
    mem: Rc<MemMod>,
    offset: goff,
    size: goff,
    map: MemMap<goff>,
    perm: Perm,
}

impl MemSlice {
    pub fn new(mem: Rc<MemMod>, offset: goff, size: goff, perm: Perm) -> Self {
        MemSlice {
            mem,
            offset,
            size,
            map: MemMap::new(offset, size),
            perm,
        }
    }

    pub fn in_reserved_mem(&self) -> bool {
        self.mem.reserved
    }

    pub fn derive(&self) -> Result<MemGate, Error> {
        self.mem
            .gate
            .derive(self.offset, self.size as usize, self.perm)
    }

    pub fn derive_with(&self, off: goff, size: usize) -> Result<MemGate, Error> {
        self.mem.gate.derive(self.offset + off, size, self.perm)
    }

    pub fn allocate(&mut self, size: goff, align: goff) -> Result<goff, Error> {
        self.map.allocate(size, align)
    }

    pub fn free(&mut self, addr: goff, size: goff) {
        self.map.free(addr, size);
    }

    pub fn addr(&self) -> GlobAddr {
        self.mem.addr + self.offset
    }

    pub fn sel(&self) -> Selector {
        self.mem.gate.sel()
    }

    pub fn capacity(&self) -> goff {
        self.size
    }

    pub fn available(&self) -> goff {
        self.map.size().0
    }
}

impl fmt::Display for MemSlice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MemSlice[{:?} .. {:?}, {:?}]",
            self.mem.addr + self.offset,
            self.mem.addr + self.offset + (self.size - 1),
            self.perm,
        )
    }
}

impl fmt::Debug for MemSlice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MemSlice[mod: {:?}, available: {} MiB, perm: {:?}, map: {:?}]",
            self.mem,
            self.map.size().0 / (1024 * 1024),
            self.perm,
            self.map
        )
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Allocation {
    slice_id: usize,
    addr: goff,
    size: goff,
}

impl Allocation {
    pub fn new(slice_id: usize, addr: goff, size: goff) -> Self {
        Allocation {
            slice_id,
            addr,
            size,
        }
    }

    pub fn slice_id(&self) -> usize {
        self.slice_id
    }

    pub fn addr(&self) -> goff {
        self.addr
    }

    pub fn size(&self) -> goff {
        self.size
    }
}

impl fmt::Debug for Allocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Alloc[slice={}, addr={:#x}, size={:#x}]",
            self.slice_id, self.addr, self.size
        )
    }
}

#[derive(Default)]
pub struct MemPool {
    slices: Vec<MemSlice>,
}

impl MemPool {
    pub fn slices(&self) -> &Vec<MemSlice> {
        &self.slices
    }

    pub fn capacity(&self) -> goff {
        self.slices.iter().fold(0, |total, m| total + m.capacity())
    }

    pub fn available(&self) -> goff {
        self.slices.iter().fold(0, |total, m| total + m.available())
    }

    pub fn mem_cap(&self, idx: usize) -> Selector {
        self.slices[idx].mem.gate.sel()
    }

    fn add(&mut self, s: MemSlice) {
        self.slices.push(s)
    }

    pub fn allocate_slice(&mut self, size: goff) -> Result<MemSlice, Error> {
        let alloc = self.allocate(size)?;
        let slice = &self.slices[alloc.slice_id];
        Ok(MemSlice::new(
            slice.mem.clone(),
            alloc.addr,
            alloc.size,
            Perm::RWX,
        ))
    }

    pub fn allocate(&mut self, size: goff) -> Result<Allocation, Error> {
        let align = if size >= cfg::LPAGE_SIZE as goff {
            cfg::LPAGE_SIZE as goff
        }
        else {
            cfg::PAGE_SIZE as goff
        };

        for (id, s) in self.slices.iter_mut().enumerate() {
            if s.mem.reserved {
                continue;
            }

            if let Ok(addr) = s.allocate(size, align) {
                let alloc = Allocation::new(id, addr, size);
                log!(LogFlags::ResMngMem, "Allocated {:?}", alloc);
                return Ok(alloc);
            }
        }
        Err(Error::new(Code::OutOfMem))
    }

    pub fn free(&mut self, alloc: Allocation) {
        let s = &mut self.slices[alloc.slice_id];
        log!(LogFlags::ResMngMem, "Freeing {:?}", alloc);
        if !s.mem.reserved {
            s.free(alloc.addr, alloc.size);
        }
    }
}

impl fmt::Debug for MemPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "MemPool[size: {} MiB, available: {} MiB, slices: [",
            self.capacity() / (1024 * 1024),
            self.available() / (1024 * 1024)
        )?;
        for m in &self.slices {
            writeln!(f, "  {:?}", m)?;
        }
        write!(f, "]]")
    }
}
