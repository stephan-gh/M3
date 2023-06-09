/*
 * Copyright (C) 2020-2022 Nils Asmussen, Barkhausen Institut
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

use base::errors::Error;
use base::mem::{GlobAddr, GlobOff, MemMap};
use base::tcu::TileId;
use core::fmt;

#[allow(dead_code)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum MemType {
    KERNEL,
    ROOT,
    USER,
    OCCUPIED,
}

pub struct MemMod {
    gaddr: GlobAddr,
    size: GlobOff,
    map: MemMap<GlobOff>,
    ty: MemType,
}

impl MemMod {
    pub fn new(ty: MemType, tile: TileId, offset: GlobOff, size: GlobOff) -> Self {
        MemMod {
            gaddr: GlobAddr::new_with(tile, offset),
            size,
            map: MemMap::new(0, size),
            ty,
        }
    }

    pub fn mem_type(&self) -> MemType {
        self.ty
    }

    pub fn addr(&self) -> GlobAddr {
        self.gaddr
    }

    pub fn largest_contiguous(&self) -> Option<GlobOff> {
        self.map.largest_contiguous()
    }

    pub fn capacity(&self) -> GlobOff {
        self.size
    }

    pub fn available(&self) -> GlobOff {
        self.map.size().0
    }

    pub fn allocate(&mut self, size: GlobOff, align: GlobOff) -> Result<GlobAddr, Error> {
        self.map.allocate(size, align).map(|addr| self.gaddr + addr)
    }

    pub fn free(&mut self, addr: GlobAddr, size: GlobOff) -> bool {
        if addr.tile() == self.gaddr.tile() {
            self.map.free(addr.offset() - self.gaddr.offset(), size);
            true
        }
        else {
            false
        }
    }
}

impl fmt::Debug for MemMod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MemMod[type: {:?}, addr: {}, size: {} MiB, available: {} MiB, map: {:?}]",
            self.ty,
            self.gaddr,
            self.capacity() / (1024 * 1024),
            self.available() / (1024 * 1024),
            self.map
        )
    }
}
