/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
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

use cfg_if::cfg_if;
use core::fmt;
use core::ops;

use crate::arch::tcu::TileId;
use crate::errors::{Code, Error};
use crate::goff;
use crate::kif::{PageFlags, Perm};
use crate::tcu::EpId;

pub type Phys = u64;

/// Represents a global address, which is a combination of a tile id and an offset within the tile.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct GlobAddr {
    val: u64,
}

cfg_if! {
    if #[cfg(not(target_vendor = "host"))] {
        const TILE_SHIFT: u64 = 56;
        const TILE_OFFSET: u64 = 0x80;
    }
    else {
        const TILE_SHIFT: u64 = 48;
        const TILE_OFFSET: u64 = 0x80;
    }
}

impl GlobAddr {
    /// Creates a new global address from the given raw value
    pub fn new(addr: u64) -> GlobAddr {
        GlobAddr { val: addr }
    }

    /// Creates a new global address from the given tile id and offset
    pub fn new_with(tile: TileId, off: goff) -> GlobAddr {
        Self::new(((0x80 + tile as u64) << TILE_SHIFT) | off)
    }

    /// Creates a new global address from the given physical address
    ///
    /// The function assumes that the given physical address is accessible through a PMP EP and uses
    /// the current configuration of this PMP EP to translate the physical address into a global
    /// address.
    pub fn new_from_phys(_phys: Phys) -> Result<GlobAddr, Error> {
        cfg_if! {
            if #[cfg(target_vendor = "host")] {
                Err(Error::new(Code::NotSup))
            }
            else {
                use crate::io::log;
                use crate::tcu::TCU;

                let phys = _phys - crate::cfg::MEM_OFFSET as Phys;
                let epid = ((phys >> 30) & 0x3) as EpId;
                let off = phys & 0x3FFF_FFFF;
                let res = TCU::unpack_mem_ep(epid)
                    .map(|(tile, addr, _, _)| GlobAddr::new_with(tile, addr + off))
                    .ok_or_else(|| Error::new(Code::InvArgs));
                log!(log::TRANSLATE, "Translated {:#x} to {:?}", phys, res);
                res
            }
        }
    }

    /// Returns the raw value
    pub fn raw(self) -> u64 {
        self.val
    }

    /// Returns whether a tile id is set
    pub fn has_tile(self) -> bool {
        self.val >= (TILE_OFFSET << TILE_SHIFT)
    }

    /// Returns the tile id
    pub fn tile(self) -> TileId {
        ((self.val >> TILE_SHIFT) - 0x80) as TileId
    }

    /// Returns the offset
    pub fn offset(self) -> goff {
        (self.val & ((1 << TILE_SHIFT) - 1)) as goff
    }

    /// Translates this global address to a physical address based on the PMP EPs.
    ///
    /// The function assumes that the callers tile has a physical-memory protection (PMP) endpoint
    /// (EP) that allows the caller to access this memory. Therefore, it walks over all PMP EPs to
    /// check which EP provides access to the address and translates it into the corresponding
    /// physical address.
    pub fn to_phys(self, _access: PageFlags) -> Result<Phys, Error> {
        cfg_if! {
            if #[cfg(target_vendor = "host")] {
                Err(Error::new(Code::NotSup))
            }
            else {
                self.to_phys_with(_access, crate::tcu::TCU::unpack_mem_ep)
            }
        }
    }

    /// Translates this global address to a physical address based on the given function to retrieve
    /// a PMP EP.
    ///
    /// Similarly to `to_phys`, `to_phys_with` translates from this global address to the physical
    /// address, but instead of reading the PMP EPs, it calls `get_ep` for every EP id.
    pub fn to_phys_with<F>(self, _access: PageFlags, _get_ep: F) -> Result<Phys, Error>
    where
        F: Fn(EpId) -> Option<(TileId, u64, u64, Perm)>,
    {
        cfg_if! {
            if #[cfg(target_vendor = "host")] {
                Err(Error::new(Code::NotSup))
            }
            else {
                use crate::io::log;
                use crate::tcu::PMEM_PROT_EPS;

                // find memory EP that contains the address
                for ep in 0..PMEM_PROT_EPS as EpId {
                    if let Some((tile, addr, size, perm)) = _get_ep(ep) {
                        log!(
                            log::TRANSLATE,
                            "Translating {:?}: considering EP{} with tile={}, addr={:#x}, size={:#x}",
                            self,
                            ep,
                            tile,
                            addr,
                            size
                        );

                        // does the EP contain this address?
                        if self.tile() == tile && self.offset() >= addr && self.offset() < addr + size {
                            let flags = PageFlags::from(perm);

                            // check access permissions
                            if _access.contains(PageFlags::R) && !flags.contains(PageFlags::R) {
                                return Err(Error::new(Code::NoPerm));
                            }
                            if _access.contains(PageFlags::W) && !flags.contains(PageFlags::W) {
                                return Err(Error::new(Code::NoPerm));
                            }

                            let phys = crate::cfg::MEM_OFFSET as Phys
                                + ((ep as Phys) << 30 | (self.offset() - addr));
                            log!(log::TRANSLATE, "Translated {:?} to {:#x}", self, phys);
                            return Ok(phys);
                        }
                    }
                }
                Err(Error::new(Code::InvArgs))
            }
        }
    }
}

impl fmt::Debug for GlobAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.has_tile() {
            write!(f, "G[Tile{}+{:#x}]", self.tile(), self.offset())
        }
        // we need global addresses without tile prefix for, e.g., the TCU MMIO region
        else {
            write!(f, "G[{:#x}]", self.raw())
        }
    }
}

impl ops::Add<goff> for GlobAddr {
    type Output = GlobAddr;

    fn add(self, rhs: goff) -> Self::Output {
        GlobAddr::new(self.val + rhs)
    }
}
