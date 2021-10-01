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

use cfg_if::cfg_if;
use core::fmt;
use core::ops;

use crate::arch::tcu::PEId;
use crate::errors::{Code, Error};
use crate::goff;
use crate::kif::{PageFlags, Perm};
use crate::tcu::EpId;

pub type Phys = u64;

/// Represents a global address, which is a combination of a PE id and an offset within the PE.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct GlobAddr {
    val: u64,
}

cfg_if! {
    if #[cfg(not(target_vendor = "host"))] {
        const PE_SHIFT: u64 = 56;
        const PE_OFFSET: u64 = 0x80;
    }
    else {
        const PE_SHIFT: u64 = 48;
        const PE_OFFSET: u64 = 0x0;
    }
}

impl GlobAddr {
    /// Creates a new global address from the given raw value
    pub fn new(addr: u64) -> GlobAddr {
        GlobAddr { val: addr }
    }

    /// Creates a new global address from the given PE id and offset
    pub fn new_with(pe: PEId, off: goff) -> GlobAddr {
        Self::new(((0x80 + pe as u64) << PE_SHIFT) | off)
    }

    /// Creates a new global address from the given physical address
    ///
    /// The function assumes that the given physical address is accessible through a PMP EP and uses
    /// the current configuration of this PMP EP to translate the physical address into a global
    /// address.
    #[cfg(not(target_vendor = "host"))]
    pub fn new_from_phys(phys: Phys) -> Result<GlobAddr, Error> {
        cfg_if! {
            if #[cfg(target_vendor = "host")] {
                Err(Error::new(Code::NotSup))
            }
            else {
                use crate::io::log;
                use crate::tcu::TCU;

                let phys = phys - crate::cfg::MEM_OFFSET as Phys;
                let epid = ((phys >> 30) & 0x3) as EpId;
                let off = phys & 0x3FFF_FFFF;
                let res = TCU::unpack_mem_ep(epid)
                    .map(|(pe, addr, _, _)| GlobAddr::new_with(pe, addr + off))
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

    /// Returns whether a PE id is set
    pub fn has_pe(self) -> bool {
        self.val >= (PE_OFFSET << PE_SHIFT)
    }

    /// Returns the PE id
    pub fn pe(self) -> PEId {
        ((self.val >> PE_SHIFT) - 0x80) as PEId
    }

    /// Returns the offset
    pub fn offset(self) -> goff {
        (self.val & ((1 << PE_SHIFT) - 1)) as goff
    }

    /// Translates this global address to a physical address based on the PMP EPs.
    ///
    /// The function assumes that the callers PE has a physical-memory protection (PMP) endpoint
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
        F: Fn(EpId) -> Option<(PEId, u64, u64, Perm)>,
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
                    if let Some((pe, addr, size, perm)) = _get_ep(ep) {
                        log!(
                            log::TRANSLATE,
                            "Translating {:?}: considering EP{} with pe={}, addr={:#x}, size={:#x}",
                            self,
                            ep,
                            pe,
                            addr,
                            size
                        );

                        // does the EP contain this address?
                        if self.pe() == pe && self.offset() >= addr && self.offset() < addr + size {
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
    #[allow(clippy::absurd_extreme_comparisons)]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.has_pe() {
            write!(f, "G[PE{}+{:#x}]", self.pe(), self.offset())
        }
        // we need global addresses without PE prefix for, e.g., the TCU MMIO region
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
