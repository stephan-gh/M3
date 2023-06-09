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

use base::cell::LazyStaticRefCell;
use base::kif::{PageFlags, Perm};
use base::libc;
use base::mem::VirtAddr;
use base::tcu;
use base::tmif;

use isr::{ISRArch, ISR};

use crate::runtime::paging;
use crate::tiles;

static STATE: LazyStaticRefCell<isr::State> = LazyStaticRefCell::default();

pub fn init() {
    STATE.set(isr::State::default());
    ISR::init(&mut STATE.borrow_mut());
    ISR::reg_tm_calls(tmcall);
    ISR::enable_irqs();
}

pub extern "C" fn tmcall(state: &mut isr::State) -> *mut libc::c_void {
    assert!(state.r[isr::TMC_ARG0] == tmif::Operation::TranslFault.into());

    let virt = VirtAddr::from(state.r[isr::TMC_ARG1]);
    let access = Perm::from_bits_truncate(state.r[isr::TMC_ARG2] as u32);
    let access = PageFlags::from(access);

    let (phys, flags) = paging::translate(virt, access);
    if (!flags.bits() & access.bits()) != 0 {
        panic!("Pagefault during PT walk for {} (flags={:?})", virt, flags);
    }

    tcu::TCU::insert_tlb(tiles::KERNEL_ID, virt, phys, flags).unwrap();

    state as *mut _ as *mut libc::c_void
}
