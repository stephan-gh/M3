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

use base::cell::StaticUnsafeCell;
use base::col::Vec;
use base::goff;
use base::mem::GlobAddr;
use core::fmt;

use crate::mem::{MemMod, MemType};

pub struct MainMemory {
    mods: Vec<MemMod>,
}

pub struct Allocation {
    gaddr: GlobAddr,
    size: goff,
}

impl Allocation {
    pub fn new(gaddr: GlobAddr, size: goff) -> Self {
        Allocation { gaddr, size }
    }

    pub fn claim(&mut self) {
        self.size = 0;
    }

    pub fn global(&self) -> GlobAddr {
        self.gaddr
    }

    pub fn size(&self) -> goff {
        self.size
    }
}

impl fmt::Debug for Allocation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Alloc[addr={:?}, size={:#x}]", self.gaddr, self.size)
    }
}

impl Drop for Allocation {
    fn drop(&mut self) {
        if self.size > 0 {
            get().free(self);
        }
    }
}

impl MainMemory {
    const fn new() -> Self {
        MainMemory { mods: Vec::new() }
    }

    pub fn mods(&self) -> &[MemMod] {
        &self.mods
    }

    pub fn add(&mut self, m: MemMod) {
        self.mods.push(m)
    }

    pub fn allocate(
        &mut self,
        mtype: MemType,
        size: goff,
        align: goff,
    ) -> Result<Allocation, base::errors::Error> {
        use base::errors::{Code, Error};

        for m in &mut self.mods {
            if m.mem_type() != mtype {
                continue;
            }

            if let Ok(gaddr) = m.allocate(size, align) {
                klog!(MEM, "Allocated {:#x} bytes at {:?}", size, gaddr);
                return Ok(Allocation::new(gaddr, size));
            }
        }
        Err(Error::new(Code::OutOfMem))
    }

    pub fn free(&mut self, alloc: &Allocation) {
        for m in &mut self.mods {
            if m.free(alloc.gaddr, alloc.size) {
                klog!(MEM, "Freed {:#x} bytes at {:?}", alloc.size, alloc.gaddr);
                break;
            }
        }
    }

    pub fn capacity(&self) -> goff {
        self.mods
            .iter()
            .fold(0, |total, ref m| total + m.capacity())
    }

    pub fn available(&self) -> goff {
        self.mods.iter().fold(0, |total, ref m| {
            if m.mem_type() != MemType::OCCUPIED {
                total + m.available()
            }
            else {
                total
            }
        })
    }
}

impl fmt::Debug for MainMemory {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(
            f,
            "size: {} MiB, available: {} MiB, mods: [",
            self.capacity() / (1024 * 1024),
            self.available() / (1024 * 1024)
        )?;
        for m in &self.mods {
            writeln!(f, "  {:?}", m)?;
        }
        write!(f, "]")
    }
}

static MEM: StaticUnsafeCell<MainMemory> = StaticUnsafeCell::new(MainMemory::new());

pub fn get() -> &'static mut MainMemory {
    MEM.get_mut()
}
