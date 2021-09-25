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

use base::cell::LazyStaticRefCell;
use base::cfg;
use base::kif::{PageFlags, Perm};
use base::libc;
use base::pexif;
use base::tcu;

use crate::arch::paging;
use crate::pes;

static STATE: LazyStaticRefCell<isr::State> = LazyStaticRefCell::default();

pub fn init() {
    STATE.set(isr::State::default());
    isr::init(&mut STATE.borrow_mut());
    isr::init_pexcalls(pexcall);
    isr::enable_irqs();
}

pub extern "C" fn pexcall(state: &mut isr::State) -> *mut libc::c_void {
    assert!(state.r[isr::PEXC_ARG0] == pexif::Operation::TRANSL_FAULT.val as usize);

    let virt = state.r[isr::PEXC_ARG1] as usize;
    let access = Perm::from_bits_truncate(state.r[isr::PEXC_ARG2] as u32);
    let flags = PageFlags::from(access);

    let pte = paging::translate(virt, flags);
    if (!(pte & 0xF) & flags.bits()) != 0 {
        panic!("Pagefault during PT walk for {:#x} (PTE={:#x})", virt, pte);
    }

    let phys = pte & !(cfg::PAGE_MASK as u64);
    let flags = PageFlags::from_bits_truncate(pte & cfg::PAGE_MASK as u64);
    tcu::TCU::insert_tlb(pes::KERNEL_ID, virt, phys, flags).unwrap();

    state as *mut _ as *mut libc::c_void
}
