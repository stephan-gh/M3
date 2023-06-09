/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2020 Nils Asmussen, Barkhausen Institut
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
use core::ops;

use num_traits::PrimInt;

use crate::col::DList;
use crate::errors::{Code, Error};
use crate::util::math;

struct Area<T: PrimInt> {
    addr: T,
    size: T,
}

impl<T: PrimInt> Area<T> {
    pub fn new(addr: T, size: T) -> Self {
        Area { addr, size }
    }
}

impl<T: PrimInt + fmt::LowerHex> fmt::Debug for Area<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Area[addr={:#x}, size={:#x}]", self.addr, self.size)
    }
}

/// The memory map, allowing allocs and frees of memory areas
pub struct MemMap<T: PrimInt> {
    areas: DList<Area<T>>,
}

impl<T: PrimInt + ops::AddAssign + ops::SubAssign> MemMap<T> {
    /// Creates a new memory map from `addr` to `addr`+`size`.
    pub fn new(addr: T, size: T) -> Self {
        let mut areas = DList::new();
        areas.push_back(Area::new(addr, size));
        MemMap { areas }
    }

    /// Allocates a region of `size` bytes, aligned by `align`.
    pub fn allocate(&mut self, size: T, align: T) -> Result<T, Error> {
        // find an area with sufficient space
        let mut it = self.areas.iter_mut();
        let a: Option<&mut Area<T>> = loop {
            match it.next() {
                None => break None,
                Some(a) => {
                    let diff = math::round_up(a.addr, align) - a.addr;
                    if a.size > diff && a.size - diff >= size {
                        break Some(a);
                    }
                },
            }
        };

        match a {
            None => Err(Error::new(Code::OutOfMem)),
            Some(a) => {
                // if we need to do some alignment, create a new area in front of a
                let diff = math::round_up(a.addr, align) - a.addr;
                if diff != T::zero() {
                    it.insert_before(Area::new(a.addr, diff));
                    a.addr += diff;
                    a.size -= diff;
                }

                // take it from the front
                let res = a.addr;
                a.size -= size;
                a.addr += size;

                // if the area is empty now, remove it
                if a.size == T::zero() {
                    it.remove();
                }

                Ok(res)
            },
        }
    }

    /// Free's the given memory region defined by `addr` and `size`.
    pub fn free(&mut self, addr: T, size: T) {
        // find the area behind ours
        let mut it = self.areas.iter_mut();
        let n: Option<&mut Area<T>> = loop {
            match it.next() {
                None => break None,
                Some(n) => {
                    if addr <= n.addr {
                        break Some(n);
                    }
                },
            }
        };

        let res = {
            let p: Option<&mut Area<T>> = it.peek_prev();
            match (p, n) {
                // merge with prev and next
                (Some(ref mut p), Some(ref n))
                    if p.addr + p.size == addr && addr + size == n.addr =>
                {
                    p.size += size + n.size;
                    1
                }

                // merge with prev
                (Some(ref mut p), _) if p.addr + p.size == addr => {
                    p.size += size;
                    0
                },

                // merge with next
                (_, Some(ref mut n)) if addr + size == n.addr => {
                    n.addr -= size;
                    n.size += size;
                    0
                },

                (_, _) => 2,
            }
        };

        if res == 1 {
            it.remove();
        }
        else if res == 2 {
            it.insert_before(Area::new(addr, size));
        }
    }

    /// Returns the size of the largest contiguous free space
    pub fn largest_contiguous(&self) -> Option<T> {
        self.areas
            .iter()
            .max_by(|a, b| a.size.cmp(&b.size))
            .map(|a| a.size)
    }

    /// Returns a pair of the remaining space and the number of areas.
    pub fn size(&self) -> (T, usize) {
        let mut total = T::zero();
        for a in self.areas.iter() {
            total += a.size;
        }
        (total, self.areas.len())
    }
}

impl<T: PrimInt + fmt::LowerHex> fmt::Debug for MemMap<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "[")?;
        for a in &self.areas {
            writeln!(f, "    {:?}", a)?;
        }
        write!(f, "  ]")
    }
}
