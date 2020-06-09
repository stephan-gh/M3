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

use base::errors::Error;
use base::kif::PageFlags;

use vma;
use vpe;

pub type State = isr::State;

pub const PEXC_ARG0: usize = 0; // r0
pub const PEXC_ARG1: usize = 1; // r1
pub const PEXC_ARG2: usize = 2; // r2

pub fn init_state(state: &mut State, entry: usize, sp: usize) {
    state.r[1] = 0xDEAD_BEEF; // don't set the stackpointer in crt0
    state.pc = entry;
    state.sp = sp;
    state.cpsr = 0x10; // user mode
    state.lr = 0;
}

pub fn init(stack: usize) {
    isr::init(stack);
    for i in 0..=7 {
        match isr::Vector::from(i) {
            isr::Vector::SWI => isr::reg(i, crate::pexcall),
            isr::Vector::PREFETCH_ABORT => isr::reg(i, crate::mmu_pf),
            isr::Vector::DATA_ABORT => isr::reg(i, crate::mmu_pf),
            isr::Vector::IRQ => isr::reg(i, crate::tcu_irq),
            _ => isr::reg(i, crate::unexpected_irq),
        }
    }
}

pub fn forget_fpu(_vpe_id: vpe::Id) {
    // no FPU support
}

pub fn disable_fpu() {
    // no FPU support
}

pub fn handle_mmu_pf(state: &mut State) -> Result<(), Error> {
    let (virt, perm) = if state.vec == isr::Vector::DATA_ABORT.val {
        let dfar: usize;
        let dfsr: usize;
        unsafe {
            llvm_asm!("mrc p15, 0, $0, c6, c0, 0; mrc p15, 0, $1, c5, c0, 0" : "=r"(dfar), "=r"(dfsr));
        }
        (
            dfar,
            if dfsr & 0x800 != 0 {
                PageFlags::RW
            }
            else {
                PageFlags::R
            },
        )
    }
    else {
        let ifar: usize;
        unsafe {
            llvm_asm!("mrc p15, 0, $0, c6, c0, 2" : "=r"(ifar));
        }
        (ifar, PageFlags::RX)
    };

    vma::handle_pf(state, virt, perm, state.pc)
}
