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
use base::libc;
use base::tcu;

use crate::arch::paging;

pub fn init() {
    isr::init(cfg::STACK_BOTTOM + cfg::STACK_SIZE / 2);
    isr::reg(isr::TCU_ISR, tcu_irq);
    isr::enable_irqs();
}

fn handle_xlate(req: tcu::CoreXlateReq) {
    let pte = paging::translate(req.virt, req.perm);
    if (!(pte & 0xF) & req.perm.bits()) != 0 {
        panic!("Pagefault during PT walk for {:#x} (PTE={:#x})", req.virt, pte);
    }

    tcu::TCU::set_xlate_resp(pte);
}

pub extern "C" fn tcu_irq(state: &mut isr::State) -> *mut libc::c_void {
    tcu::TCU::clear_irq(tcu::IRQ::CORE_REQ);

    match tcu::TCU::get_core_req() {
        Some(tcu::CoreReq::Xlate(r)) => handle_xlate(r),
        _ => assert!(false),
    }

    state as *mut _ as *mut libc::c_void
}
