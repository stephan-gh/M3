/*
 * Copyright (C) 2020-2021 Nils Asmussen, Barkhausen Institut
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

use base::cell::{RefMut, StaticRefCell};
use base::col::Vec;
use base::goff;
use base::io::LogFlags;
use base::log;
use base::mem::GlobAddr;
use core::fmt;

use crate::mem::{MemMod, MemType};

pub struct MainMemory {
    mods: Vec<MemMod>,
}

#[derive(Copy, Clone)]
pub struct Allocation {
    gaddr: GlobAddr,
    size: goff,
}

impl Allocation {
    pub fn new(gaddr: GlobAddr, size: goff) -> Self {
        Allocation { gaddr, size }
    }

    pub fn global(&self) -> GlobAddr {
        self.gaddr
    }

    pub fn size(&self) -> goff {
        self.size
    }
}

impl fmt::Debug for Allocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Alloc[addr={}, size={:#x}]", self.gaddr, self.size)
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
                log!(
                    LogFlags::KernMem,
                    "Allocated {:#x} bytes at {}",
                    size,
                    gaddr
                );
                return Ok(Allocation::new(gaddr, size));
            }
        }
        Err(Error::new(Code::OutOfMem))
    }

    pub fn free(&mut self, alloc: &Allocation) {
        for m in &mut self.mods {
            if m.free(alloc.gaddr, alloc.size) {
                log!(
                    LogFlags::KernMem,
                    "Freed {:#x} bytes at {}",
                    alloc.size,
                    alloc.gaddr
                );
                break;
            }
        }
    }

    pub fn largest_contiguous(&self, mtype: MemType) -> Option<goff> {
        let mut max = None;
        for m in &self.mods {
            if m.mem_type() == mtype {
                let m_max = m.largest_contiguous();
                if m_max.unwrap_or(0) > max.unwrap_or(0) {
                    max = m_max;
                }
            }
        }
        max
    }

    pub fn capacity(&self) -> goff {
        self.mods.iter().fold(0, |total, m| total + m.capacity())
    }

    pub fn available(&self) -> goff {
        self.mods.iter().fold(0, |total, m| {
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

static MEM: StaticRefCell<MainMemory> = StaticRefCell::new(MainMemory::new());

pub fn borrow_mut() -> RefMut<'static, MainMemory> {
    MEM.borrow_mut()
}
