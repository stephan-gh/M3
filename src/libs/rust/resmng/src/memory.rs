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
use m3::cap::Selector;
use m3::cfg;
use m3::col::Vec;
use m3::com::MemGate;
use m3::errors::{Code, Error};
use m3::goff;
use m3::mem::MemMap;
use m3::rc::Rc;

pub struct MemMod {
    gate: MemGate,
    addr: goff,
    size: goff,
    reserved: bool,
}

impl MemMod {
    pub fn new(sel: Selector, addr: goff, size: goff, reserved: bool) -> Self {
        MemMod {
            gate: MemGate::new_bind(sel),
            addr,
            size,
            reserved,
        }
    }

    pub fn capacity(&self) -> goff {
        self.size
    }
}

impl fmt::Debug for MemMod {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "MemMod[sel: {}, res: {}, addr: {:#x}, size: {} MiB]",
            self.gate.sel(),
            self.reserved,
            self.addr,
            self.size / (1024 * 1024),
        )
    }
}

#[derive(Default)]
pub struct MemModCon {
    mods: Vec<Rc<MemMod>>,
    cur_mod: usize,
    cur_off: goff,
}

impl MemModCon {
    pub fn add(&mut self, m: Rc<MemMod>) {
        self.mods.push(m);
    }

    pub fn capacity(&self) -> goff {
        self.mods.iter().fold(0, |total, ref m| {
            if !m.reserved {
                total + m.capacity()
            }
            else {
                total
            }
        })
    }

    pub fn find_mem(&mut self, phys: goff, size: goff) -> Result<MemSlice, Error> {
        for m in &self.mods {
            if m.reserved && phys >= m.addr && phys + size <= m.addr + m.capacity() {
                return Ok(MemSlice::new(m.clone(), phys - m.addr, size));
            }
        }
        Err(Error::new(Code::InvArgs))
    }

    pub fn alloc_mem(&mut self, size: goff) -> Result<MemSlice, Error> {
        while self.cur_mod < self.mods.len() {
            if let Some(sl) = self.get_slice(size) {
                return Ok(sl);
            }
        }
        Err(Error::new(Code::NoSpace))
    }

    pub fn alloc_pool(&mut self, mut size: goff) -> Result<MemPool, Error> {
        let mut res = MemPool::default();
        while size > 0 && self.cur_mod < self.mods.len() {
            if let Some(sl) = self.get_slice(size) {
                size -= sl.size;
                self.cur_off += sl.size;
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

    fn get_slice(&mut self, size: goff) -> Option<MemSlice> {
        let m = &self.mods[self.cur_mod];
        if m.reserved || self.cur_off == m.capacity() {
            self.cur_mod += 1;
            self.cur_off = 0;
            return None;
        }

        let avail = m.capacity() - self.cur_off;
        let amount = cmp::min(avail, size);
        Some(MemSlice::new(m.clone(), self.cur_off, amount))
    }
}

pub struct MemSlice {
    mem: Rc<MemMod>,
    size: goff,
    map: MemMap,
}

impl MemSlice {
    pub fn new(mem: Rc<MemMod>, offset: goff, size: goff) -> Self {
        MemSlice {
            mem,
            size,
            map: MemMap::new(offset, size),
        }
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

impl fmt::Debug for MemSlice {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "MemSlice[mod: {:?}, available: {} MiB, map: {:?}]",
            self.mem,
            self.map.size().0 / (1024 * 1024),
            self.map
        )
    }
}

#[derive(Copy, Clone)]
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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
    pub fn capacity(&self) -> goff {
        self.slices
            .iter()
            .fold(0, |total, ref m| total + m.capacity())
    }

    pub fn available(&self) -> goff {
        self.slices
            .iter()
            .fold(0, |total, ref m| total + m.available())
    }

    pub fn mem_cap(&self, idx: usize) -> Selector {
        self.slices[idx].mem.gate.sel()
    }

    pub fn add(&mut self, s: MemSlice) {
        self.slices.push(s)
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

            if let Ok(addr) = s.map.allocate(size, align) {
                let alloc = Allocation::new(id, addr, size);
                log!(crate::LOG_MEM, "Allocated {:?}", alloc);
                return Ok(alloc);
            }
        }
        Err(Error::new(Code::OutOfMem))
    }

    pub fn allocate_at(&mut self, phys: goff, size: goff) -> Result<Allocation, Error> {
        for (id, s) in self.slices.iter().enumerate() {
            if s.mem.reserved && phys >= s.mem.addr && phys + size <= s.mem.addr + s.capacity() {
                let alloc = Allocation::new(id, phys, size);
                log!(crate::LOG_MEM, "Allocated {:?}", alloc);
                return Ok(alloc);
            }
        }
        Err(Error::new(Code::NoPerm))
    }

    pub fn free(&mut self, alloc: Allocation) {
        let s = &mut self.slices[alloc.slice_id];
        log!(crate::LOG_MEM, "Freeing {:?}", alloc);
        if !s.mem.reserved {
            s.map.free(alloc.addr, alloc.size);
        }
    }
}

impl fmt::Debug for MemPool {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
