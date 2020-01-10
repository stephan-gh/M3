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

//! The boot information that the kernel passes to root

use core::fmt;
use core::intrinsics;
use core::iter;
use util;

const MAX_MEMS: usize = 4;

/// A memory region
#[repr(C, packed)]
#[derive(Default, Copy, Clone, Debug)]
pub struct Mem {
    addr: u64,
    size: u64,
}

impl Mem {
    /// Creates a new memory region of given size.
    pub fn new(addr: u64, size: u64, reserved: bool) -> Self {
        Mem {
            addr,
            size: size | (reserved as u64),
        }
    }

    /// Returns the start address of the memory region
    pub fn addr(&self) -> u64 {
        self.addr
    }

    /// Returns the size of the memory region
    pub fn size(self) -> u64 {
        self.size & !1
    }

    /// Returns true if the region is reserved, that is, not usable by applications
    pub fn reserved(self) -> bool {
        (self.size & 1) == 1
    }
}

/// The boot information
#[repr(C, packed)]
#[derive(Default, Copy, Clone, Debug)]
pub struct Info {
    /// The number of boot modules
    pub mod_count: u64,
    /// The size of all boot modules
    pub mod_size: u64,
    /// The number of PEs
    pub pe_count: u64,
    /// the base address of the memory areas for PEs
    pub pe_mem_base: u64,
    /// the size of the memory area per PE
    pub pe_mem_size: u64,
    /// The memory regions
    pub mems: [Mem; MAX_MEMS],
}

/// A boot module
#[repr(C, packed)]
pub struct Mod {
    /// The address of the module
    pub addr: u64,
    /// The size of the module
    pub size: u64,
    namelen: u64,
    name: [i8],
}

impl Mod {
    /// Returns the name and arguments of the module
    pub fn name(&self) -> &'static str {
        unsafe { util::cstr_to_str(self.name.as_ptr()) }
    }
}

impl fmt::Debug for Mod {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(
            f,
            "Mod[addr: {:#x}, size: {:#x}, name: {}]",
            { self.addr },
            { self.size },
            self.name()
        )
    }
}

/// An iterator for the boot modules
pub struct ModIterator {
    addr: usize,
    end: usize,
}

impl ModIterator {
    /// Creates a new iterator for the boot modules at `addr`..`addr`+`len`.
    pub fn new(addr: usize, len: usize) -> Self {
        ModIterator {
            addr,
            end: addr + len,
        }
    }
}

impl iter::Iterator for ModIterator {
    type Item = &'static Mod;

    fn next(&mut self) -> Option<Self::Item> {
        if self.addr == self.end {
            None
        }
        else {
            unsafe {
                // build a slice to be able to get a pointer to Mod (it has a flexible member)
                let m: *const Mod = intrinsics::transmute([self.addr as usize, 0usize]);
                // now build a slice for the module with the actual length by reading <namelen>
                let slice: [usize; 2] = [self.addr, (*m).namelen as usize];
                // move forward
                self.addr += util::size_of::<u64>() * 3 + (*m).namelen as usize;
                // return reference
                Some(intrinsics::transmute(slice))
            }
        }
    }
}
