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

#[no_mangle]
pub extern "C" fn to_mmu_pte(_pte: dtu::PTE) -> u64 {
    unimplemented!();
}

#[no_mangle]
pub extern "C" fn to_dtu_pte(_pte: u64) -> dtu::PTE {
    unimplemented!();
}

#[no_mangle]
pub extern "C" fn noc_to_phys(_noc: u64) -> u64 {
    unimplemented!();
}

#[no_mangle]
pub extern "C" fn get_pte_addr(_virt: u64, _level: u32) -> u64 {
    unimplemented!();
}

#[no_mangle]
pub extern "C" fn get_pte_at(_virt: u64, _level: u32) -> u64 {
    unimplemented!();
}

#[no_mangle]
pub extern "C" fn get_pte(_virt: u64, _perm: u64) -> u64 {
    unimplemented!();
}
