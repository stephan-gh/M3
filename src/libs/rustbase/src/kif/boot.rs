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
use core::intrinsics;
use core::iter;
use util;

#[repr(C, packed)]
#[derive(Default, Copy, Clone, Debug)]
pub struct Info {
    pub mod_count: u64,
    pub mod_size: u64,
    pub pe_count: u64,
}

#[repr(C, packed)]
pub struct Mod {
    pub addr: u64,
    pub size: u64,
    namelen: u64,
    name: [i8],
}

impl Mod {
    pub fn name(&self) -> &'static str {
        unsafe {
            util::cstr_to_str(self.name.as_ptr())
        }
    }
}

impl fmt::Debug for Mod {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "Mod[addr: {:#x}, size: {:#x}, name: {}]",
               {self.addr}, {self.size}, self.name())
    }
}

pub struct ModIterator {
    addr: usize,
    end: usize,
}

impl ModIterator {
    pub fn new(addr: usize, len: usize) -> Self {
        ModIterator {
            addr: addr,
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
