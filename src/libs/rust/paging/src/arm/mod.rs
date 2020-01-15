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

use base::dtu;
use base::errors::Error;
use base::goff;

use crate::{AllocFrameFunc, XlatePtFunc};

pub type PTE = u32;

#[no_mangle]
pub extern "C" fn to_dtu_pte(_pte: PTE) -> dtu::PTE {
    unimplemented!();
}

#[no_mangle]
pub extern "C" fn get_addr_space() -> PTE {
    // TODO implement me
    0
}

#[no_mangle]
pub extern "C" fn set_addr_space(_addr: PTE) {
    // TODO implement me
}

#[no_mangle]
pub extern "C" fn noc_to_phys(noc: u64) -> u64 {
    // TODO implement me
    noc
}

#[no_mangle]
pub extern "C" fn phys_to_noc(phys: u64) -> u64 {
    // TODO implement me
    phys
}

#[no_mangle]
pub extern "C" fn get_pte_at(_virt: usize, _level: u32) -> PTE {
    unimplemented!();
}

#[no_mangle]
pub extern "C" fn get_pte(_virt: usize, _perm: u64) -> PTE {
    unimplemented!();
}

#[no_mangle]
pub extern "C" fn map_pages(
    _vpe: u64,
    _virt: usize,
    _phys: goff,
    _pages: usize,
    _perm: u64,
    _alloc_frame: AllocFrameFunc,
    _xlate_pt: XlatePtFunc,
    _root: goff,
) {
}

pub struct AddrSpace {
    pub vpe: u64,
    pub root: goff,
}

impl AddrSpace {
    pub fn new(vpe: u64, root: goff, _xlate_pt: XlatePtFunc, _alloc_frame: AllocFrameFunc) -> Self {
        AddrSpace {
            vpe,
            root,
        }
    }

    pub fn init(&self) {
    }

    pub fn map_pages(
        &self,
        _virt: usize,
        _phys: goff,
        _pages: usize,
        _perm: dtu::PTEFlags,
    ) -> Result<(), Error> {
        unimplemented!();
    }
}
