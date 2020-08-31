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

use base::cfg;
use base::kif::PageFlags;
use base::libc;
use base::tcu;

use crate::arch::paging;

pub fn init() {
    isr::init(cfg::STACK_BOTTOM + cfg::STACK_SIZE / 2);
    isr::reg(isr::TCU_ISR, tcu_irq);
    isr::enable_irqs();
}

fn handle_xlate(xlate_req: tcu::Reg) {
    let virt = (xlate_req & 0xFFFF_FFFF_FFFF) as usize & !cfg::PAGE_MASK;
    let perm = PageFlags::from_bits_truncate((xlate_req >> 2) & PageFlags::RW.bits());

    let pte = paging::translate(virt, perm);
    if (!(pte & 0xF) & perm.bits()) != 0 {
        panic!("Pagefault during PT walk for {:#x} (PTE={:#x})", virt, pte);
    }

    tcu::TCU::set_core_req(pte);
}

pub extern "C" fn tcu_irq(state: &mut isr::State) -> *mut libc::c_void {
    tcu::TCU::clear_irq(tcu::IRQ::CORE_REQ);

    let core_req = tcu::TCU::get_core_req();
    if core_req != 0 {
        assert!((core_req & 0x1) == 0);
        handle_xlate(core_req);
    }

    state as *mut _ as *mut libc::c_void
}
