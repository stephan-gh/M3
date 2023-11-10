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
use m3::cell::StaticRefCell;
use m3::cfg;
use m3::com::{MemCap, MemGate};
use m3::errors::Error;
use m3::mem::{self, GlobOff};

static ZEROS: mem::AlignedBuf<{ cfg::PAGE_SIZE }> = mem::AlignedBuf::new_zeroed();
static BUF: StaticRefCell<mem::AlignedBuf<{ cfg::PAGE_SIZE }>> =
    StaticRefCell::new(mem::AlignedBuf::new_zeroed());

pub fn copy_block(src: &MemGate, dst: &MemGate, src_off: GlobOff, size: GlobOff) {
    let mut buf = BUF.borrow_mut();
    let pages = size / cfg::PAGE_SIZE as GlobOff;
    for i in 0..pages {
        src.read(&mut buf[..], src_off + i * cfg::PAGE_SIZE as GlobOff)
            .unwrap();
        dst.write(&buf[..], i * cfg::PAGE_SIZE as GlobOff).unwrap();
    }
}

pub fn clear_block(mem: &MemGate, size: GlobOff) {
    let pages = size / cfg::PAGE_SIZE as GlobOff;
    for i in 0..pages {
        mem.write(&ZEROS[..], i * cfg::PAGE_SIZE as GlobOff)
            .unwrap();
    }
}

pub struct PhysMem {
    mcap: MemCap,
    owner_mem: Option<(Selector, mem::VirtAddr)>,
}

impl PhysMem {
    pub fn new(owner_mem: (Selector, mem::VirtAddr), mem: MemCap) -> Result<Self, Error> {
        Ok(PhysMem {
            mcap: mem,
            owner_mem: Some(owner_mem),
        })
    }

    pub fn new_with_mem(owner_mem: (Selector, mem::VirtAddr), mem: MemCap) -> Self {
        PhysMem {
            mcap: mem,
            owner_mem: Some(owner_mem),
        }
    }

    pub fn new_bind(owner_mem: (Selector, mem::VirtAddr), sel: Selector) -> Self {
        PhysMem {
            mcap: MemCap::new_bind(sel),
            owner_mem: Some(owner_mem),
        }
    }

    pub fn mem_sel(&self) -> Selector {
        self.mcap.sel()
    }

    pub fn request_gate(&self) -> Result<MemGate, Error> {
        MemGate::new_bind(self.mcap.sel())
    }

    pub fn replace_mem(&mut self, mem: MemCap) -> MemCap {
        mem::replace(&mut self.mcap, mem)
    }

    pub fn owner_mem(&self) -> Option<(Selector, mem::VirtAddr)> {
        self.owner_mem
    }

    pub fn set_owner(&mut self, act: Selector, virt: mem::VirtAddr) {
        self.owner_mem = Some((act, virt));
    }

    pub fn remove_owner(&mut self) {
        self.owner_mem = None;
    }
}
